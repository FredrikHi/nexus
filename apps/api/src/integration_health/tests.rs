//! Database-level tests for evaluation and the transition log.
//!
//! The judging rules themselves are unit tested in `evaluator`, without a
//! database. These cover what only real Postgres can show: policy resolution,
//! the upsert, and when a transition is and is not recorded.

use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::telemetry::TelemetryStatus;

use super::model::HealthStatus;
use super::{repository, service};

const DEFAULT_ORG: Uuid = Uuid::from_u128(1);

async fn seed_integration(pool: &PgPool, name: &str, monitored: bool) -> Uuid {
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
              destination_component_id, integration_type_id, monitoring_enabled)
           VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id"#,
        DEFAULT_ORG,
        name,
        name,
        components[0],
        components[1],
        kind,
        monitored
    )
    .fetch_one(pool)
    .await
    .expect("integration")
}

/// Writes `count` events, `failures` of them unhealthy, all just now.
async fn events(pool: &PgPool, integration: Uuid, count: i64, failures: i64) {
    for n in 0..count {
        let status = if n < failures {
            TelemetryStatus::Failure
        } else {
            TelemetryStatus::Success
        };
        sqlx::query!(
            r#"INSERT INTO telemetry_events
                 (organization_id, integration_id, occurred_at, status, duration_ms)
               VALUES ($1,$2,$3,$4,$5)"#,
            DEFAULT_ORG,
            integration,
            Utc::now() - Duration::seconds(n),
            status.as_str(),
            50_i32
        )
        .execute(pool)
        .await
        .expect("insert event");
    }
}

async fn transition_count(pool: &PgPool, integration: Uuid) -> i64 {
    sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM health_transitions WHERE integration_id = $1"#,
        integration
    )
    .fetch_one(pool)
    .await
    .expect("count transitions")
}

/// A first evaluation records both a verdict and the transition into it, with
/// no previous status.
#[sqlx::test]
async fn first_evaluation_records_a_verdict_and_a_transition(pool: PgPool) {
    let i = seed_integration(&pool, "fresh", true).await;
    events(&pool, i, 20, 0).await;

    let result = service::evaluate(&pool, DEFAULT_ORG).await.expect("evaluate");
    assert_eq!(result.evaluated, 1);
    assert_eq!(result.changed, 1, "the first verdict is itself a change");

    let health = service::get(&pool, i).await.expect("health");
    assert_eq!(health.status, HealthStatus::Healthy);
    assert_eq!(health.event_count, 20);

    let history = service::transitions(&pool, i, None).await.expect("transitions");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].from_status, None, "nothing preceded the first verdict");
    assert_eq!(history[0].to_status, HealthStatus::Healthy);
}

/// Re-evaluating an unchanged integration must not append history, and must not
/// move `since`: that field means when the state began, not when it was last
/// looked at, and incidents will depend on it.
#[sqlx::test]
async fn a_stable_status_appends_no_history_and_does_not_move_since(pool: PgPool) {
    let i = seed_integration(&pool, "stable", true).await;
    events(&pool, i, 20, 0).await;

    service::evaluate(&pool, DEFAULT_ORG).await.expect("first pass");
    let first = service::get(&pool, i).await.expect("health");

    service::evaluate(&pool, DEFAULT_ORG).await.expect("second pass");
    let second = service::get(&pool, i).await.expect("health");

    assert_eq!(second.status, first.status);
    assert_eq!(second.since, first.since, "since must not move on an unchanged status");
    assert!(second.evaluated_at >= first.evaluated_at, "but evaluated_at must advance");
    assert_eq!(transition_count(&pool, i).await, 1, "no duplicate history");
}

/// A real change appends history.
#[sqlx::test]
async fn a_status_change_is_recorded(pool: PgPool) {
    let i = seed_integration(&pool, "degrading", true).await;
    events(&pool, i, 20, 0).await;
    service::evaluate(&pool, DEFAULT_ORG).await.expect("healthy pass");

    // Twenty more events, all failing: half the window is now bad, well past
    // the default 20% unhealthy threshold.
    events(&pool, i, 20, 20).await;
    let result = service::evaluate(&pool, DEFAULT_ORG).await.expect("unhealthy pass");
    assert_eq!(result.changed, 1);

    let health = service::get(&pool, i).await.expect("health");
    assert_eq!(health.status, HealthStatus::Unhealthy);

    let history = service::transitions(&pool, i, None).await.expect("transitions");
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].to_status, HealthStatus::Unhealthy, "newest first");
    assert_eq!(history[0].from_status, Some(HealthStatus::Healthy));
}

/// A per-integration policy overrides the organization default.
#[sqlx::test]
async fn a_per_integration_policy_overrides_the_org_default(pool: PgPool) {
    let strict = seed_integration(&pool, "strict", true).await;
    let lenient = seed_integration(&pool, "lenient", true).await;

    // 10% errors: past the strict override, but only degraded under the
    // organization default of 5% degraded and 20% unhealthy.
    events(&pool, strict, 20, 2).await;
    events(&pool, lenient, 20, 2).await;

    sqlx::query!(
        r#"INSERT INTO health_policies
             (organization_id, integration_id, error_rate_degraded, error_rate_unhealthy)
           VALUES ($1, $2, 0.01, 0.05)"#,
        DEFAULT_ORG,
        strict
    )
    .execute(&pool)
    .await
    .expect("insert policy");

    service::evaluate(&pool, DEFAULT_ORG).await.expect("evaluate");

    assert_eq!(
        service::get(&pool, strict).await.expect("strict").status,
        HealthStatus::Unhealthy,
        "10% errors is past the override's 5% unhealthy threshold"
    );
    assert_eq!(
        service::get(&pool, lenient).await.expect("lenient").status,
        HealthStatus::Degraded,
        "the same traffic only degrades under the org default"
    );
}

/// Integrations with monitoring turned off are not evaluated at all.
#[sqlx::test]
async fn unmonitored_integrations_are_skipped(pool: PgPool) {
    let watched = seed_integration(&pool, "watched", true).await;
    let ignored = seed_integration(&pool, "ignored", false).await;
    events(&pool, watched, 20, 0).await;
    events(&pool, ignored, 20, 20).await;

    let result = service::evaluate(&pool, DEFAULT_ORG).await.expect("evaluate");
    assert_eq!(result.evaluated, 1, "only the monitored integration is judged");

    assert!(service::get(&pool, ignored).await.is_err(), "no record for an unmonitored one");
    assert_eq!(transition_count(&pool, ignored).await, 0);
}

/// Retention drops whole partitions and reports how many. Nothing is old
/// enough in a fresh database, so the honest answer is zero.
#[sqlx::test]
async fn pruning_reports_what_it_dropped(pool: PgPool) {
    let dropped = repository::prune_telemetry(&pool, 90).await.expect("prune");
    assert_eq!(dropped, 0);
}
