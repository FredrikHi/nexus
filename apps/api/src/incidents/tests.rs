//! Lifecycle tests for the reconciler.
//!
//! Severity derivation is unit tested in `model` without a database. These
//! cover the parts that need real Postgres: the partial unique index, the
//! open/escalate/resolve transitions, and the timeline.

use sqlx::PgPool;
use uuid::Uuid;

use crate::integration_health;

use super::dto::{AddNote, ListIncidentsQuery};
use super::model::{IncidentEventKind, IncidentStatus, Severity};
use super::service;

const DEFAULT_ORG: Uuid = Uuid::from_u128(1);

async fn seed_integration(pool: &PgPool, name: &str, criticality: &str) -> Uuid {
    let system = sqlx::query_scalar!(
        "INSERT INTO systems (organization_id, name, slug) VALUES ($1,$2,$3) RETURNING id",
        DEFAULT_ORG,
        name,
        format!("{name}-sys")
    )
    .fetch_one(pool)
    .await
    .expect("system");

    let mut components = Vec::new();
    for side in ["src", "dst"] {
        components.push(
            sqlx::query_scalar!(
                "INSERT INTO components (system_id, name, slug) VALUES ($1,$2,$3) RETURNING id",
                system,
                format!("{name}-{side}"),
                format!("{name}-{side}")
            )
            .fetch_one(pool)
            .await
            .expect("component"),
        );
    }

    let kind = sqlx::query_scalar!("SELECT id FROM integration_types WHERE key = 'HTTP'")
        .fetch_one(pool)
        .await
        .expect("integration type");

    sqlx::query_scalar!(
        r#"INSERT INTO integrations
             (organization_id, name, slug, source_component_id,
              destination_component_id, integration_type_id, criticality)
           VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id"#,
        DEFAULT_ORG,
        name,
        name,
        components[0],
        components[1],
        kind,
        criticality
    )
    .fetch_one(pool)
    .await
    .expect("integration")
}

/// Writes `count` events, the first `failures` of them unhealthy.
///
/// One statement via generate_series rather than a loop: these tests need
/// thousands of rows to move an error rate across a threshold, and a round
/// trip per row makes the suite crawl.
///
/// Events are spaced a millisecond apart so a large batch still lands inside
/// the 15 minute evaluation window. Spacing them by seconds would push most of
/// a two thousand event batch out of the window and quietly change the rate
/// the test is trying to set.
async fn events(pool: &PgPool, integration: Uuid, count: i64, failures: i64) {
    sqlx::query!(
        r#"INSERT INTO telemetry_events
             (organization_id, integration_id, occurred_at, status, duration_ms)
           SELECT $1, $2,
                  now() - make_interval(secs => g * 0.001),
                  CASE WHEN g < $3 THEN 'FAILURE' ELSE 'SUCCESS' END,
                  50
           FROM generate_series(0, $4 - 1) AS g"#,
        DEFAULT_ORG,
        integration,
        failures as i32,
        count as i32
    )
    .execute(pool)
    .await
    .expect("insert events");
}

/// Evaluate health, then reconcile incidents, exactly as the worker does.
async fn pass(pool: &PgPool) -> super::model::ReconcileResult {
    integration_health::evaluate_org(pool, DEFAULT_ORG)
        .await
        .expect("evaluate");
    service::reconcile(pool, DEFAULT_ORG)
        .await
        .expect("reconcile")
}

fn open_query() -> ListIncidentsQuery {
    ListIncidentsQuery {
        integration_id: None,
        severity: None,
        status: None,
        only_open: Some(true),
        limit: None,
    }
}

/// The whole arc: a healthy integration breaks, gets acknowledged, gets worse,
/// then recovers, with every step on the timeline.
#[sqlx::test]
async fn an_incident_opens_escalates_and_resolves_with_health(pool: PgPool) {
    let i = seed_integration(&pool, "payments", "CRITICAL").await;

    // Healthy first: nothing should open.
    events(&pool, i, 30, 0).await;
    let result = pass(&pool).await;
    assert_eq!(result.opened, 0, "a healthy integration opens no incident");

    // 10% errors: degraded, and CRITICAL business criticality lifts it to MAJOR.
    events(&pool, i, 30, 6).await;
    let result = pass(&pool).await;
    assert_eq!(result.opened, 1);

    let open = service::list(&pool, DEFAULT_ORG, open_query())
        .await
        .expect("list");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].severity, Severity::Major);
    assert_eq!(open[0].status, IncidentStatus::Open);
    let incident_id = open[0].id;

    // A person picks it up. It stays open: only recovery resolves.
    let detail = service::acknowledge(&pool, DEFAULT_ORG, incident_id, "fredrik")
        .await
        .expect("acknowledge");
    assert_eq!(detail.incident.status, IncidentStatus::Acknowledged);
    assert!(
        detail.incident.resolved_at.is_none(),
        "acknowledging must not resolve"
    );

    // It gets worse: past the unhealthy threshold, so CRITICAL.
    events(&pool, i, 60, 60).await;
    let result = pass(&pool).await;
    assert_eq!(result.escalated, 1);
    let detail = service::get(&pool, DEFAULT_ORG, incident_id)
        .await
        .expect("get");
    assert_eq!(detail.incident.severity, Severity::Critical);

    // Recovery. 66 failures are already banked, so the window needs well over
    // 1320 events in total before the rate falls back under 5%.
    events(&pool, i, 2000, 0).await;
    let result = pass(&pool).await;
    assert_eq!(result.resolved, 1);

    let detail = service::get(&pool, DEFAULT_ORG, incident_id)
        .await
        .expect("get");
    assert_eq!(detail.incident.status, IncidentStatus::Resolved);
    assert!(detail.incident.resolved_at.is_some());
    assert!(service::list(&pool, DEFAULT_ORG, open_query())
        .await
        .expect("list")
        .is_empty());

    let kinds: Vec<IncidentEventKind> = detail.timeline.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        vec![
            IncidentEventKind::Opened,
            IncidentEventKind::Acknowledged,
            IncidentEventKind::Escalated,
            IncidentEventKind::Resolved,
        ]
    );
}

/// Reconciling repeatedly must not open a second incident. The partial unique
/// index enforces this in the database, not just in the reconciler.
#[sqlx::test]
async fn only_one_incident_stays_open_per_integration(pool: PgPool) {
    let i = seed_integration(&pool, "flaky", "LOW").await;
    events(&pool, i, 30, 15).await;

    pass(&pool).await;
    pass(&pool).await;
    pass(&pool).await;

    let open = service::list(&pool, DEFAULT_ORG, open_query())
        .await
        .expect("list");
    assert_eq!(
        open.len(),
        1,
        "repeated passes must not duplicate the incident"
    );

    let detail = service::get(&pool, DEFAULT_ORG, open[0].id)
        .await
        .expect("get");
    let opened = detail
        .timeline
        .iter()
        .filter(|e| e.kind == IncidentEventKind::Opened)
        .count();
    assert_eq!(opened, 1);
}

/// An integration that goes quiet while broken has not been fixed, so its
/// incident stays open rather than silently disappearing.
#[sqlx::test]
async fn going_quiet_does_not_resolve_an_incident(pool: PgPool) {
    let i = seed_integration(&pool, "silent", "MEDIUM").await;
    events(&pool, i, 30, 15).await;
    pass(&pool).await;
    assert_eq!(
        service::list(&pool, DEFAULT_ORG, open_query())
            .await
            .expect("list")
            .len(),
        1
    );

    // Past the measurement window, but well inside the twelve hours a verdict
    // is trusted for. Going quiet does not launder a failure: the last thing
    // actually observed was a broken integration, so it stays broken.
    sqlx::query!("UPDATE telemetry_events SET occurred_at = occurred_at - INTERVAL '5 hours'")
        .execute(&pool)
        .await
        .expect("age the events");

    pass(&pool).await;

    let health = integration_health::health_of(&pool, DEFAULT_ORG, i)
        .await
        .expect("health");
    assert_eq!(
        health, "UNHEALTHY",
        "a quiet integration keeps the verdict it earned"
    );

    let open = service::list(&pool, DEFAULT_ORG, open_query())
        .await
        .expect("list");
    assert_eq!(open.len(), 1, "the incident must stay open");

    // Now past the point where the last verdict can be stood behind. The
    // status becomes honest again, and the incident still does not close:
    // losing sight of something is not the same as fixing it.
    sqlx::query!("UPDATE telemetry_events SET occurred_at = occurred_at - INTERVAL '10 hours'")
        .execute(&pool)
        .await
        .expect("age the events further");

    pass(&pool).await;

    let health = integration_health::health_of(&pool, DEFAULT_ORG, i)
        .await
        .expect("health");
    assert_eq!(health, "UNKNOWN", "past the stale window, nothing is known");

    let open = service::list(&pool, DEFAULT_ORG, open_query())
        .await
        .expect("list");
    assert_eq!(
        open.len(),
        1,
        "the incident must stay open while health is unknown"
    );
}

/// Severity never falls while an incident is open: the peak is the useful
/// number in a postmortem.
#[sqlx::test]
async fn severity_does_not_fall_while_open(pool: PgPool) {
    let i = seed_integration(&pool, "recovering", "LOW").await;

    // Badly broken: UNHEALTHY on a LOW integration is MAJOR.
    events(&pool, i, 30, 30).await;
    pass(&pool).await;
    let open = service::list(&pool, DEFAULT_ORG, open_query())
        .await
        .expect("list");
    assert_eq!(open[0].severity, Severity::Major);
    let incident_id = open[0].id;

    // Improves to merely degraded, which alone would rate MINOR.
    events(&pool, i, 270, 0).await;
    pass(&pool).await;

    let detail = service::get(&pool, DEFAULT_ORG, incident_id)
        .await
        .expect("get");
    assert_eq!(
        detail.incident.status,
        IncidentStatus::Open,
        "still not healthy"
    );
    assert_eq!(
        detail.incident.severity,
        Severity::Major,
        "severity must not fall"
    );
}

/// Acknowledging twice is a conflict rather than a silent no-op.
#[sqlx::test]
async fn acknowledging_twice_conflicts(pool: PgPool) {
    let i = seed_integration(&pool, "twice", "LOW").await;
    events(&pool, i, 30, 15).await;
    pass(&pool).await;
    let id = service::list(&pool, DEFAULT_ORG, open_query())
        .await
        .expect("list")[0]
        .id;

    service::acknowledge(&pool, DEFAULT_ORG, id, "a")
        .await
        .expect("first acknowledge");

    let second = service::acknowledge(&pool, DEFAULT_ORG, id, "b").await;
    assert!(
        matches!(second, Err(crate::error::ApiError::Conflict(_))),
        "{second:?}"
    );
}

/// Notes stay allowed after resolution, so a postmortem can live with the
/// incident it describes.
#[sqlx::test]
async fn notes_can_be_added_after_resolution(pool: PgPool) {
    let i = seed_integration(&pool, "postmortem", "LOW").await;
    events(&pool, i, 30, 15).await;
    pass(&pool).await;
    let id = service::list(&pool, DEFAULT_ORG, open_query())
        .await
        .expect("list")[0]
        .id;

    events(&pool, i, 600, 0).await;
    pass(&pool).await;

    let detail = service::add_note(
        &pool,
        DEFAULT_ORG,
        id,
        "fredrik",
        AddNote {
            message: "Upstream deploy rolled back.".into(),
        },
    )
    .await
    .expect("add note");

    assert_eq!(detail.incident.status, IncidentStatus::Resolved);
    assert_eq!(
        detail.timeline.last().expect("a note").kind,
        IncidentEventKind::Note
    );
}
