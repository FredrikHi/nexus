//! Integration tests for the derived trace view.
//!
//! These insert straight into telemetry_events rather than going through
//! ingest: traces are a read model over that table, so the shortest honest
//! setup is to write the rows the view reads.

use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::telemetry::TelemetryStatus;

use super::dto::ListTracesQuery;
use super::service;

const DEFAULT_ORG: Uuid = Uuid::from_u128(1);

/// One system, two components, one integration. Returns the integration id.
async fn seed_integration(pool: &PgPool, name: &str) -> Uuid {
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
              destination_component_id, integration_type_id)
           VALUES ($1,$2,$3,$4,$5,$6) RETURNING id"#,
        DEFAULT_ORG,
        name,
        name,
        components[0],
        components[1],
        kind
    )
    .fetch_one(pool)
    .await
    .expect("integration")
}

/// Writes one span into a trace, `seconds_ago` before now.
async fn span(pool: &PgPool, trace: &str, integration: Uuid, status: TelemetryStatus, seconds_ago: i64) {
    sqlx::query!(
        r#"INSERT INTO telemetry_events
             (organization_id, integration_id, occurred_at, status, duration_ms, trace_id)
           VALUES ($1,$2,$3,$4,$5,$6)"#,
        DEFAULT_ORG,
        integration,
        Utc::now() - Duration::seconds(seconds_ago),
        status.as_str(),
        10_i32,
        trace
    )
    .execute(pool)
    .await
    .expect("insert span");
}

fn query() -> ListTracesQuery {
    ListTracesQuery {
        integration_id: None,
        from: None,
        to: None,
        only_errors: None,
        limit: None,
    }
}

/// A trace is only as healthy as its unhealthiest hop, and a genuine error
/// outranks a deliberate refusal.
#[sqlx::test]
async fn status_rolls_up_by_severity(pool: PgPool) {
    let i = seed_integration(&pool, "rollup").await;

    // Each trace mixes a SUCCESS with one worse status.
    span(&pool, "mixed-failure", i, TelemetryStatus::Success, 60).await;
    span(&pool, "mixed-failure", i, TelemetryStatus::Failure, 59).await;
    span(&pool, "mixed-failure", i, TelemetryStatus::Timeout, 58).await;

    span(&pool, "mixed-timeout", i, TelemetryStatus::Success, 50).await;
    span(&pool, "mixed-timeout", i, TelemetryStatus::Rejected, 49).await;
    span(&pool, "mixed-timeout", i, TelemetryStatus::Timeout, 48).await;

    span(&pool, "only-rejected", i, TelemetryStatus::Success, 40).await;
    span(&pool, "only-rejected", i, TelemetryStatus::Rejected, 39).await;

    span(&pool, "all-good", i, TelemetryStatus::Success, 30).await;

    let traces = service::list(&pool, query()).await.expect("list traces");
    let by_id = |id: &str| {
        traces
            .iter()
            .find(|t| t.trace_id == id)
            .unwrap_or_else(|| panic!("trace {id} missing"))
    };

    assert_eq!(by_id("mixed-failure").status, TelemetryStatus::Failure, "FAILURE outranks TIMEOUT");
    assert_eq!(by_id("mixed-timeout").status, TelemetryStatus::Timeout, "TIMEOUT outranks REJECTED");
    assert_eq!(by_id("only-rejected").status, TelemetryStatus::Rejected);
    assert_eq!(by_id("all-good").status, TelemetryStatus::Success);

    // REJECTED is not an error, for the same reason it is excluded from the
    // telemetry error rate: it is the caller's fault, not the dependency's.
    assert_eq!(by_id("only-rejected").error_count, 0);
    assert_eq!(by_id("mixed-failure").error_count, 2, "failure and timeout both count");
}

/// Filtering by an integration must return the WHOLE trace, not only the spans
/// belonging to that integration: the interesting hop is usually the one before
/// or after the one that broke.
#[sqlx::test]
async fn filtering_by_integration_keeps_the_whole_trace(pool: PgPool) {
    let first = seed_integration(&pool, "first").await;
    let second = seed_integration(&pool, "second").await;

    span(&pool, "two-hop", first, TelemetryStatus::Success, 20).await;
    span(&pool, "two-hop", second, TelemetryStatus::Failure, 19).await;
    // A trace that never touches `second`, so it must not be returned.
    span(&pool, "one-hop", first, TelemetryStatus::Success, 10).await;

    let mut q = query();
    q.integration_id = Some(second);
    let traces = service::list(&pool, q).await.expect("list traces");

    assert_eq!(traces.len(), 1, "only the trace touching `second` matches");
    let trace = &traces[0];
    assert_eq!(trace.trace_id, "two-hop");
    assert_eq!(trace.span_count, 2, "both hops are included, not just the matching one");
    assert_eq!(trace.integration_count, 2);
}

/// only_errors keeps traces containing a FAILURE or TIMEOUT and drops the rest.
#[sqlx::test]
async fn only_errors_drops_healthy_and_rejected_traces(pool: PgPool) {
    let i = seed_integration(&pool, "errors").await;

    span(&pool, "broken", i, TelemetryStatus::Failure, 30).await;
    span(&pool, "slow", i, TelemetryStatus::Timeout, 20).await;
    span(&pool, "refused", i, TelemetryStatus::Rejected, 15).await;
    span(&pool, "fine", i, TelemetryStatus::Success, 10).await;

    let mut q = query();
    q.only_errors = Some(true);
    let traces = service::list(&pool, q).await.expect("list traces");

    let mut ids: Vec<&str> = traces.iter().map(|t| t.trace_id.as_str()).collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["broken", "slow"]);
}
