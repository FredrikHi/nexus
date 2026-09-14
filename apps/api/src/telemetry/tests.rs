//! Integration tests for the telemetry path.
//!
//! `#[sqlx::test]` creates a fresh, isolated, fully migrated database per test
//! and drops it afterwards, so these run against real Postgres, real
//! constraints and real partitions rather than a mock.

use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::api_keys::AuthenticatedKey;
use crate::error::ApiError;

use super::dto::{IngestBatch, IngestEvent, SummaryQuery};
use super::model::TelemetryStatus;
use super::service;

const DEFAULT_ORG: Uuid = Uuid::from_u128(1);

/// Builds a minimal landscape: one system, two components, one integration.
async fn seed_integration(pool: &PgPool, org: Uuid, name: &str) -> Uuid {
    sqlx::query!(
        "INSERT INTO organizations (id, name, slug) VALUES ($1, $2, $3) ON CONFLICT (id) DO NOTHING",
        org,
        format!("org-{org}"),
        format!("org-{org}")
    )
    .execute(pool)
    .await
    .expect("insert org");

    let system = sqlx::query_scalar!(
        r#"INSERT INTO systems (organization_id, name, slug)
           VALUES ($1, $2, $3) RETURNING id"#,
        org,
        name,
        format!("{name}-system")
    )
    .fetch_one(pool)
    .await
    .expect("insert system");

    let mut components = Vec::new();
    for side in ["source", "destination"] {
        let id = sqlx::query_scalar!(
            r#"INSERT INTO components (system_id, name, slug)
               VALUES ($1, $2, $3) RETURNING id"#,
            system,
            format!("{name}-{side}"),
            format!("{name}-{side}")
        )
        .fetch_one(pool)
        .await
        .expect("insert component");
        components.push(id);
    }

    let integration_type =
        sqlx::query_scalar!("SELECT id FROM integration_types WHERE key = 'HTTP'")
            .fetch_one(pool)
            .await
            .expect("seeded integration type");

    sqlx::query_scalar!(
        r#"INSERT INTO integrations
             (organization_id, name, slug, source_component_id,
              destination_component_id, integration_type_id)
           VALUES ($1,$2,$3,$4,$5,$6) RETURNING id"#,
        org,
        name,
        name,
        components[0],
        components[1],
        integration_type
    )
    .fetch_one(pool)
    .await
    .expect("insert integration")
}

fn auth(org: Uuid, environment_id: Option<Uuid>) -> AuthenticatedKey {
    // The id is arbitrary here: ingest authorises on the organization,
    // not on which key presented it.
    AuthenticatedKey {
        api_key_id: Uuid::from_u128(42),
        organization_id: org,
        environment_id,
        allow_auto_create: false,
    }
}

fn event(integration_id: Uuid, status: TelemetryStatus, duration_ms: Option<i32>) -> IngestEvent {
    IngestEvent {
        integration: None,
        integration_id: Some(integration_id),
        occurred_at: None,
        status,
        duration_ms,
        trace_id: None,
        operation: None,
        status_code: None,
        error_type: None,
        error_message: None,
        payload_bytes: None,
        environment_id: None,
        metadata: serde_json::Value::Null,
    }
}

/// The same event, named by slug rather than by id.
fn event_by_slug(slug: &str, status: TelemetryStatus) -> IngestEvent {
    IngestEvent {
        integration: Some(slug.to_string()),
        integration_id: None,
        occurred_at: None,
        status,
        duration_ms: None,
        trace_id: None,
        operation: None,
        status_code: None,
        error_type: None,
        error_message: None,
        payload_bytes: None,
        environment_id: None,
        metadata: serde_json::Value::Null,
    }
}

async fn stored_events(pool: &PgPool) -> i64 {
    sqlx::query_scalar!(r#"SELECT count(*) AS "n!" FROM telemetry_events"#)
        .fetch_one(pool)
        .await
        .expect("count events")
}

/// The isolation test that matters: a credential for one organization must not
/// write events against another organization's integration, even though the
/// integration id is perfectly valid.
#[sqlx::test]
async fn a_key_cannot_write_to_another_organizations_integration(pool: PgPool) {
    let other_org = Uuid::from_u128(999);
    let theirs = seed_integration(&pool, other_org, "theirs").await;

    let result = service::ingest(
        &pool,
        auth(DEFAULT_ORG, None),
        IngestBatch {
            events: vec![event(theirs, TelemetryStatus::Success, None)],
        },
    )
    .await;

    match result {
        Err(ApiError::Validation(message)) => {
            // The id is real, so the refusal is about tenancy rather than
            // existence. From this caller's side those are the same answer,
            // deliberately: a different message would confirm the id exists
            // somewhere, which is an enumeration oracle.
            assert!(
                message.contains(&theirs.to_string()),
                "the error must name the id it refused, got: {message}"
            );
        }
        other => panic!("expected a validation error, got {other:?}"),
    }

    assert_eq!(
        stored_events(&pool).await,
        0,
        "a rejected batch must write nothing"
    );
}

/// A batch is all or nothing: one bad event must not leave the good ones behind.
#[sqlx::test]
async fn a_rejected_batch_writes_nothing(pool: PgPool) {
    let ours = seed_integration(&pool, DEFAULT_ORG, "ours").await;

    let result = service::ingest(
        &pool,
        auth(DEFAULT_ORG, None),
        IngestBatch {
            events: vec![
                event(ours, TelemetryStatus::Success, Some(10)),
                event(Uuid::from_u128(0xdead), TelemetryStatus::Success, Some(10)),
            ],
        },
    )
    .await;

    assert!(
        result.is_err(),
        "an unknown integration must reject the whole batch"
    );
    assert_eq!(stored_events(&pool).await, 0);
}

/// Events older than the retention window are refused up front, rather than
/// failing deep in Postgres with "no partition of relation found".
#[sqlx::test]
async fn events_outside_the_window_are_refused(pool: PgPool) {
    let ours = seed_integration(&pool, DEFAULT_ORG, "window").await;

    let mut stale = event(ours, TelemetryStatus::Success, None);
    stale.occurred_at = Some(Utc::now() - Duration::days(120));

    let result = service::ingest(
        &pool,
        auth(DEFAULT_ORG, None),
        IngestBatch {
            events: vec![stale],
        },
    )
    .await;

    match result {
        Err(ApiError::Validation(message)) => {
            assert!(
                message.contains("occurred_at"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected a validation error, got {other:?}"),
    }
}

/// A key pinned to an environment stamps events with it, and refuses events
/// claiming a different one.
#[sqlx::test]
async fn a_pinned_key_controls_the_environment(pool: PgPool) {
    let ours = seed_integration(&pool, DEFAULT_ORG, "pinned").await;
    let production = sqlx::query_scalar!(
        "SELECT id FROM environments WHERE slug = 'production' AND organization_id = $1",
        DEFAULT_ORG
    )
    .fetch_one(&pool)
    .await
    .expect("seeded production environment");
    let development = sqlx::query_scalar!(
        "SELECT id FROM environments WHERE slug = 'development' AND organization_id = $1",
        DEFAULT_ORG
    )
    .fetch_one(&pool)
    .await
    .expect("seeded development environment");

    let mut claiming = event(ours, TelemetryStatus::Success, None);
    claiming.environment_id = Some(development);
    let refused = service::ingest(
        &pool,
        auth(DEFAULT_ORG, Some(production)),
        IngestBatch {
            events: vec![claiming],
        },
    )
    .await;
    assert!(
        refused.is_err(),
        "a pinned key must refuse a foreign environment"
    );

    service::ingest(
        &pool,
        auth(DEFAULT_ORG, Some(production)),
        IngestBatch {
            events: vec![event(ours, TelemetryStatus::Success, None)],
        },
    )
    .await
    .expect("ingest with the pinned environment");

    let stamped = sqlx::query_scalar!("SELECT environment_id FROM telemetry_events LIMIT 1")
        .fetch_one(&pool)
        .await
        .expect("read the stored event");
    assert_eq!(
        stamped,
        Some(production),
        "the pin must supply the environment"
    );
}

/// The error rate counts FAILURE and TIMEOUT but deliberately not REJECTED: a
/// refused call is the caller's fault, and folding it in would make a noisy
/// client look like a broken dependency.
#[sqlx::test]
async fn error_rate_excludes_rejected(pool: PgPool) {
    let ours = seed_integration(&pool, DEFAULT_ORG, "rates").await;

    let mut events = Vec::new();
    for _ in 0..6 {
        events.push(event(ours, TelemetryStatus::Success, Some(10)));
    }
    events.push(event(ours, TelemetryStatus::Failure, Some(20)));
    events.push(event(ours, TelemetryStatus::Timeout, Some(30)));
    events.push(event(ours, TelemetryStatus::Rejected, Some(40)));
    events.push(event(ours, TelemetryStatus::Rejected, Some(50)));

    service::ingest(&pool, auth(DEFAULT_ORG, None), IngestBatch { events })
        .await
        .expect("ingest");

    let summary = service::summary(
        &pool,
        DEFAULT_ORG,
        SummaryQuery {
            integration_id: Some(ours),
            environment_id: None,
            from: None,
            to: None,
        },
    )
    .await
    .expect("summary");

    assert_eq!(summary.total, 10);
    assert_eq!(summary.success, 6);
    assert_eq!(summary.rejected, 2);
    // One failure and one timeout out of ten, with the two rejections ignored.
    assert!(
        (summary.error_rate - 0.2).abs() < 1e-9,
        "error_rate was {}",
        summary.error_rate
    );
}

/// The point of slugs: the same build can report to any instance, because the
/// name is stable while the generated id is not.
#[sqlx::test]
async fn a_slug_names_the_same_integration_as_its_id(pool: PgPool) {
    let integration = seed_integration(&pool, DEFAULT_ORG, "bruno-to-groq").await;

    let batch = IngestBatch {
        events: vec![
            event(integration, TelemetryStatus::Success, Some(10)),
            event_by_slug("bruno-to-groq", TelemetryStatus::Success),
        ],
    };

    let result = service::ingest(&pool, auth(DEFAULT_ORG, None), batch)
        .await
        .expect("both forms accepted");
    assert_eq!(result.accepted, 2);

    // Both landed on the same row, which is what "the same integration" means.
    let against: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM telemetry_events WHERE integration_id = $1"#,
        integration
    )
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(against, 2);
}

/// A misspelled slug should say which one, not that something somewhere failed.
#[sqlx::test]
async fn an_unknown_slug_is_named_in_the_error(pool: PgPool) {
    seed_integration(&pool, DEFAULT_ORG, "bruno-to-groq").await;

    let batch = IngestBatch {
        events: vec![event_by_slug("bruno-to-grok", TelemetryStatus::Success)],
    };

    let error = service::ingest(&pool, auth(DEFAULT_ORG, None), batch)
        .await
        .expect_err("unknown slug is refused");

    match error {
        ApiError::Validation(message) => {
            assert!(
                message.contains("bruno-to-grok"),
                "the error must name the slug it could not find, got: {message}"
            );
        }
        other => panic!("expected a validation error, got {other:?}"),
    }

    assert_eq!(stored_events(&pool).await, 0, "nothing is written");
}

/// Naming an integration twice, two different ways, is a bug in the caller
/// rather than something to guess at.
#[sqlx::test]
async fn naming_both_a_slug_and_an_id_is_refused(pool: PgPool) {
    let integration = seed_integration(&pool, DEFAULT_ORG, "bruno-to-groq").await;

    let mut both = event_by_slug("bruno-to-groq", TelemetryStatus::Success);
    both.integration_id = Some(integration);

    let error = service::ingest(
        &pool,
        auth(DEFAULT_ORG, None),
        IngestBatch { events: vec![both] },
    )
    .await
    .expect_err("ambiguous event is refused");

    assert!(matches!(error, ApiError::Validation(_)));
    assert_eq!(stored_events(&pool).await, 0);
}

/// A slug belongs to one organization. Another tenant's slug must not resolve,
/// even when the word is identical.
#[sqlx::test]
async fn a_slug_does_not_cross_organizations(pool: PgPool) {
    let other_org = Uuid::from_u128(999);
    seed_integration(&pool, other_org, "bruno-to-groq").await;
    seed_integration(&pool, DEFAULT_ORG, "something-else").await;

    let batch = IngestBatch {
        events: vec![event_by_slug("bruno-to-groq", TelemetryStatus::Success)],
    };

    let error = service::ingest(&pool, auth(DEFAULT_ORG, None), batch)
        .await
        .expect_err("another tenant's slug must not resolve");

    assert!(matches!(error, ApiError::Validation(_)));
    assert_eq!(stored_events(&pool).await, 0);
}

/// A key may be allowed to describe its own landscape, so an application can
/// report before anyone has modelled it.
fn auth_discovering(org: Uuid) -> AuthenticatedKey {
    AuthenticatedKey {
        api_key_id: Uuid::from_u128(43),
        organization_id: org,
        environment_id: None,
        allow_auto_create: true,
    }
}

#[sqlx::test]
async fn an_unknown_slug_is_created_when_the_key_allows_it(pool: PgPool) {
    // Seeding one integration also creates the organization the key belongs to.
    seed_integration(&pool, DEFAULT_ORG, "already-known").await;

    let batch = IngestBatch {
        events: vec![event_by_slug(
            "orders-to-warehouse",
            TelemetryStatus::Success,
        )],
    };

    let result = service::ingest(&pool, auth_discovering(DEFAULT_ORG), batch)
        .await
        .expect("an unknown slug is created rather than refused");
    assert_eq!(result.accepted, 1);

    let created = sqlx::query!(
        r#"SELECT i.name, s.slug AS "system_slug!", i.metadata
           FROM integrations i
           JOIN components c ON c.id = i.source_component_id
           JOIN systems s ON s.id = c.system_id
           WHERE i.organization_id = $1 AND i.slug = 'orders-to-warehouse'"#,
        DEFAULT_ORG
    )
    .fetch_one(&pool)
    .await
    .expect("the integration exists");

    // Parked under a placeholder, so the graph says what is actually known:
    // the call happens, what it runs between has not been decided.
    assert_eq!(created.system_slug, "unmapped");
    assert_eq!(created.name, "orders-to-warehouse");
    assert_eq!(created.metadata["discovered"], serde_json::json!(true));
}

#[sqlx::test]
async fn a_discovered_slug_is_created_once(pool: PgPool) {
    seed_integration(&pool, DEFAULT_ORG, "already-known").await;

    for _ in 0..3 {
        service::ingest(
            &pool,
            auth_discovering(DEFAULT_ORG),
            IngestBatch {
                events: vec![event_by_slug(
                    "orders-to-warehouse",
                    TelemetryStatus::Success,
                )],
            },
        )
        .await
        .expect("ingest");
    }

    let rows: i64 = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM integrations
           WHERE organization_id = $1 AND slug = 'orders-to-warehouse'"#,
        DEFAULT_ORG
    )
    .fetch_one(&pool)
    .await
    .expect("count");

    assert_eq!(rows, 1, "repeated ingest must reuse the integration");
    assert_eq!(stored_events(&pool).await, 3);
}

/// Auto-creation is a property of the key, not of the platform. A key without
/// it still gets a plain refusal.
#[sqlx::test]
async fn an_unknown_slug_is_still_refused_without_the_flag(pool: PgPool) {
    seed_integration(&pool, DEFAULT_ORG, "already-known").await;

    let error = service::ingest(
        &pool,
        auth(DEFAULT_ORG, None),
        IngestBatch {
            events: vec![event_by_slug(
                "orders-to-warehouse",
                TelemetryStatus::Success,
            )],
        },
    )
    .await
    .expect_err("refused");

    assert!(matches!(error, ApiError::Validation(_)));
    assert_eq!(stored_events(&pool).await, 0);
}
