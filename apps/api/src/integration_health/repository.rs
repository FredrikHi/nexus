use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::model::{
    HealthTransitionRow, IntegrationHealthRow, Measurement, ResolvedPolicy, Verdict,
};

/// One integration's resolved thresholds and its measurement over the window.
pub struct EvaluationInput {
    pub integration_id: Uuid,
    pub organization_id: Uuid,
    pub policy: ResolvedPolicy,
    pub measurement: Measurement,
}

/// Gathers everything the evaluator needs, for every monitored integration in
/// an organization, in one query.
///
/// The two LEFT JOINs onto health_policies resolve thresholds: `own` is the
/// integration's override, `fallback` the organization default, and COALESCE
/// picks the first that exists. The LEFT JOIN onto telemetry_events carries the
/// window in its ON clause, so each integration is measured over ITS OWN
/// window, and integrations with no events survive the join with zero counts
/// rather than disappearing.
pub async fn gather(db: &PgPool, org_id: Uuid) -> Result<Vec<EvaluationInput>, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT
             i.id                       AS "integration_id!",
             i.organization_id          AS "organization_id!",
             COALESCE(own.window_minutes,       fb.window_minutes,       15)   AS "window_minutes!",
             COALESCE(own.min_events,           fb.min_events,           5)    AS "min_events!",
             COALESCE(own.error_rate_degraded,  fb.error_rate_degraded,  0.05) AS "error_rate_degraded!",
             COALESCE(own.error_rate_unhealthy, fb.error_rate_unhealthy, 0.20) AS "error_rate_unhealthy!",
             COALESCE(own.p95_degraded_ms,      fb.p95_degraded_ms)            AS "p95_degraded_ms",
             COALESCE(own.p95_unhealthy_ms,     fb.p95_unhealthy_ms)           AS "p95_unhealthy_ms",
             COALESCE(own.stale_after_minutes,  fb.stale_after_minutes,  720)  AS "stale_after_minutes!",
             count(te.id)                                                     AS "event_count!",
             count(te.id) FILTER (WHERE te.status IN ('FAILURE','TIMEOUT'))    AS "error_count!",
             percentile_cont(0.95) WITHIN GROUP (ORDER BY te.duration_ms)      AS "p95_duration_ms",
             (SELECT max(occurred_at) FROM telemetry_events le
               WHERE le.integration_id = i.id)                                AS "last_event_at"
           FROM integrations i
           LEFT JOIN health_policies own ON own.integration_id = i.id
           LEFT JOIN health_policies fb
                  ON fb.organization_id = i.organization_id AND fb.integration_id IS NULL
           LEFT JOIN telemetry_events te
                  ON te.integration_id = i.id
                 AND te.occurred_at >= now() - make_interval(
                       mins => COALESCE(own.window_minutes, fb.window_minutes, 15))
           WHERE i.organization_id = $1
             AND i.monitoring_enabled
           GROUP BY i.id, i.organization_id, own.window_minutes, fb.window_minutes,
                    own.min_events, fb.min_events,
                    own.error_rate_degraded, fb.error_rate_degraded,
                    own.error_rate_unhealthy, fb.error_rate_unhealthy,
                    own.p95_degraded_ms, fb.p95_degraded_ms,
                    own.p95_unhealthy_ms, fb.p95_unhealthy_ms,
                    own.stale_after_minutes, fb.stale_after_minutes"#,
        org_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| EvaluationInput {
            integration_id: r.integration_id,
            organization_id: r.organization_id,
            policy: ResolvedPolicy {
                window_minutes: r.window_minutes,
                min_events: r.min_events,
                error_rate_degraded: r.error_rate_degraded,
                error_rate_unhealthy: r.error_rate_unhealthy,
                p95_degraded_ms: r.p95_degraded_ms,
                p95_unhealthy_ms: r.p95_unhealthy_ms,
                stale_after_minutes: r.stale_after_minutes,
            },
            measurement: Measurement {
                event_count: r.event_count,
                error_count: r.error_count,
                p95_duration_ms: r.p95_duration_ms,
                last_event_at: r.last_event_at,
            },
        })
        .collect())
}

/// The status currently recorded for each integration in an organization.
///
/// Fetched once per pass rather than per integration: the evaluator needs the
/// previous status to detect a transition, and asking per row would be an N+1.
pub async fn current_statuses(db: &PgPool, org_id: Uuid) -> Result<Vec<(Uuid, String)>, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT integration_id AS "integration_id!", status AS "status!"
           FROM integration_health
           WHERE organization_id = $1"#,
        org_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| (r.integration_id, r.status))
        .collect())
}

/// Writes the verdict for one integration.
///
/// `since` only moves when the status actually changes, so it keeps meaning
/// "when the current state began" rather than "when we last looked". That is
/// exactly what an incident needs in order to say how long something has been
/// broken.
pub async fn record(
    db: &PgPool,
    input: &EvaluationInput,
    verdict: &Verdict,
) -> Result<(), ApiError> {
    sqlx::query!(
        r#"INSERT INTO integration_health
             (integration_id, organization_id, status, since, evaluated_at, window_minutes,
              event_count, error_count, error_rate, p95_duration_ms, last_event_at, reason)
           VALUES ($1,$2,$3,now(),now(),$4,$5,$6,$7,$8,$9,$10)
           ON CONFLICT (integration_id) DO UPDATE SET
             status          = EXCLUDED.status,
             since           = CASE WHEN integration_health.status = EXCLUDED.status
                                    THEN integration_health.since ELSE now() END,
             evaluated_at    = now(),
             window_minutes  = EXCLUDED.window_minutes,
             event_count     = EXCLUDED.event_count,
             error_count     = EXCLUDED.error_count,
             error_rate      = EXCLUDED.error_rate,
             p95_duration_ms = EXCLUDED.p95_duration_ms,
             last_event_at   = EXCLUDED.last_event_at,
             reason          = EXCLUDED.reason"#,
        input.integration_id,
        input.organization_id,
        verdict.status.as_str(),
        input.policy.window_minutes,
        input.measurement.event_count,
        input.measurement.error_count,
        input.measurement.error_rate(),
        input.measurement.p95_duration_ms,
        input.measurement.last_event_at,
        verdict.reason
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Appends a transition. Incidents will be built from this log.
pub async fn record_transition(
    db: &PgPool,
    input: &EvaluationInput,
    from: Option<&str>,
    verdict: &Verdict,
) -> Result<(), ApiError> {
    sqlx::query!(
        r#"INSERT INTO health_transitions
             (integration_id, organization_id, from_status, to_status, reason,
              error_rate, event_count)
           VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
        input.integration_id,
        input.organization_id,
        from,
        verdict.status.as_str(),
        verdict.reason,
        input.measurement.error_rate(),
        input.measurement.event_count
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Current health for every integration in an organization, worst first, so a
/// dashboard shows what needs attention without sorting client-side.
pub async fn list(db: &PgPool, org_id: Uuid) -> Result<Vec<IntegrationHealthRow>, ApiError> {
    let rows = sqlx::query_as!(
        IntegrationHealthRow,
        r#"SELECT h.integration_id, i.name AS "integration_name!", i.slug AS "integration_slug!",
                  h.organization_id, h.status, h.since, h.evaluated_at,
                  h.window_minutes, h.event_count, h.error_count, h.error_rate,
                  h.p95_duration_ms, h.last_event_at, h.reason
           FROM integration_health h
           JOIN integrations i ON i.id = h.integration_id
           WHERE h.organization_id = $1
           ORDER BY CASE h.status
                      WHEN 'UNHEALTHY' THEN 0
                      WHEN 'DEGRADED'  THEN 1
                      WHEN 'UNKNOWN'   THEN 2
                      ELSE 3
                    END,
                    since ASC"#,
        org_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn find(
    db: &PgPool,
    org_id: Uuid,
    integration_id: Uuid,
) -> Result<Option<IntegrationHealthRow>, ApiError> {
    let row = sqlx::query_as!(
        IntegrationHealthRow,
        r#"SELECT h.integration_id, i.name AS "integration_name!", i.slug AS "integration_slug!",
                  h.organization_id, h.status, h.since, h.evaluated_at,
                  h.window_minutes, h.event_count, h.error_count, h.error_rate,
                  h.p95_duration_ms, h.last_event_at, h.reason
           FROM integration_health h
           JOIN integrations i ON i.id = h.integration_id
           WHERE h.organization_id = $1 AND h.integration_id = $2"#,
        org_id,
        integration_id
    )
    .fetch_optional(db)
    .await?;

    Ok(row)
}

/// The status-change log for one integration, newest first.
pub async fn transitions(
    db: &PgPool,
    org_id: Uuid,
    integration_id: Uuid,
    limit: i64,
) -> Result<Vec<HealthTransitionRow>, ApiError> {
    let rows = sqlx::query_as!(
        HealthTransitionRow,
        r#"SELECT id, integration_id, from_status, to_status, changed_at,
                  reason, error_rate, event_count
           FROM health_transitions
           WHERE organization_id = $1 AND integration_id = $2
           ORDER BY changed_at DESC
           LIMIT $3"#,
        org_id,
        integration_id,
        limit
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// Drops telemetry partitions entirely older than the retention window.
/// Returns how many were dropped.
pub async fn prune_telemetry(db: &PgPool, retention_days: i64) -> Result<i32, ApiError> {
    let dropped = sqlx::query_scalar!(
        r#"SELECT drop_telemetry_partitions_before(
             now() - make_interval(days => $1::int)
           ) AS "dropped!""#,
        retention_days as i32
    )
    .fetch_one(db)
    .await?;

    Ok(dropped)
}
