use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;
use crate::telemetry::TelemetryEvent;

use super::model::TraceSummaryRow;

// Traces read telemetry_events directly. They are a derived read model over a
// table another feature owns, which is the one place this codebase crosses a
// feature boundary on purpose: the alternative, routing every aggregate through
// the telemetry module, would grow that module's public surface for no gain.

// The status rollup below is spelled out in each query rather than shared in a
// constant: query_as! needs literal SQL, so a shared fragment cannot be
// interpolated. Precedence is FAILURE > TIMEOUT > REJECTED > SUCCESS, because a
// flow is only as healthy as its unhealthiest hop and a genuine error outranks
// a deliberate refusal.

/// Lists trace summaries, newest first.
///
/// The `integration_id` filter selects whole traces that touch the integration
/// rather than only the matching spans, which is what an operator means by
/// "show me traces through this integration": the interesting part is usually
/// the hop before or after the one that broke.
pub async fn list(
    db: &PgPool,
    org_id: Uuid,
    integration_id: Option<Uuid>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    only_errors: bool,
    limit: i64,
) -> Result<Vec<TraceSummaryRow>, ApiError> {
    let rows = sqlx::query_as!(
        TraceSummaryRow,
        r#"SELECT
             trace_id AS "trace_id!",
             min(occurred_at) AS "started_at!",
             max(occurred_at) AS "ended_at!",
             (EXTRACT(EPOCH FROM (max(occurred_at) - min(occurred_at))) * 1000)::float8
               AS "elapsed_ms!",
             count(*) AS "span_count!",
             count(DISTINCT integration_id) AS "integration_count!",
             count(*) FILTER (WHERE status IN ('FAILURE','TIMEOUT')) AS "error_count!",
             CASE
               WHEN count(*) FILTER (WHERE status = 'FAILURE')  > 0 THEN 'FAILURE'
               WHEN count(*) FILTER (WHERE status = 'TIMEOUT')  > 0 THEN 'TIMEOUT'
               WHEN count(*) FILTER (WHERE status = 'REJECTED') > 0 THEN 'REJECTED'
               ELSE 'SUCCESS'
             END AS "status!"
           FROM telemetry_events
           WHERE organization_id = $1
             AND trace_id IS NOT NULL
             AND ($2::uuid IS NULL OR trace_id IN (
                   SELECT trace_id FROM telemetry_events
                   WHERE organization_id = $1
                     AND integration_id = $2
                     AND trace_id IS NOT NULL))
             AND ($3::timestamptz IS NULL OR occurred_at >= $3)
             AND ($4::timestamptz IS NULL OR occurred_at < $4)
           GROUP BY trace_id
           HAVING $5::bool IS NOT TRUE
               OR count(*) FILTER (WHERE status IN ('FAILURE','TIMEOUT')) > 0
           ORDER BY min(occurred_at) DESC
           LIMIT $6"#,
        org_id,
        integration_id,
        from,
        to,
        only_errors,
        limit
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// The aggregate for one trace. None when the organization has no such trace.
pub async fn summary(
    db: &PgPool,
    org_id: Uuid,
    trace_id: &str,
) -> Result<Option<TraceSummaryRow>, ApiError> {
    let row = sqlx::query_as!(
        TraceSummaryRow,
        r#"SELECT
             trace_id AS "trace_id!",
             min(occurred_at) AS "started_at!",
             max(occurred_at) AS "ended_at!",
             (EXTRACT(EPOCH FROM (max(occurred_at) - min(occurred_at))) * 1000)::float8
               AS "elapsed_ms!",
             count(*) AS "span_count!",
             count(DISTINCT integration_id) AS "integration_count!",
             count(*) FILTER (WHERE status IN ('FAILURE','TIMEOUT')) AS "error_count!",
             CASE
               WHEN count(*) FILTER (WHERE status = 'FAILURE')  > 0 THEN 'FAILURE'
               WHEN count(*) FILTER (WHERE status = 'TIMEOUT')  > 0 THEN 'TIMEOUT'
               WHEN count(*) FILTER (WHERE status = 'REJECTED') > 0 THEN 'REJECTED'
               ELSE 'SUCCESS'
             END AS "status!"
           FROM telemetry_events
           WHERE organization_id = $1 AND trace_id = $2
           GROUP BY trace_id"#,
        org_id,
        trace_id
    )
    .fetch_optional(db)
    .await?;

    Ok(row)
}

/// The spans of one trace, oldest first, which is the order a flow happened in.
pub async fn spans(
    db: &PgPool,
    org_id: Uuid,
    trace_id: &str,
) -> Result<Vec<TelemetryEvent>, ApiError> {
    let rows = sqlx::query_as!(
        crate::telemetry::TelemetryEventRow,
        r#"SELECT id, organization_id, integration_id, environment_id, occurred_at,
                  status, duration_ms, trace_id, operation, status_code,
                  error_type, error_message, payload_bytes, metadata, received_at
           FROM telemetry_events
           WHERE organization_id = $1 AND trace_id = $2
           ORDER BY occurred_at ASC"#,
        org_id,
        trace_id
    )
    .fetch_all(db)
    .await?;

    rows.into_iter()
        .map(crate::telemetry::TelemetryEventRow::into_domain)
        .collect()
}
