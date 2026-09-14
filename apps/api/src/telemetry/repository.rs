use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::model::{SeriesRow, SummaryRow, TelemetryEvent, TelemetryEventRow};

/// Columns to insert, transposed from a batch of events into one array per
/// column. Postgres receives arrays, not N statements.
pub struct EventColumns {
    pub integration_id: Vec<Uuid>,
    pub environment_id: Vec<Option<Uuid>>,
    pub occurred_at: Vec<DateTime<Utc>>,
    pub status: Vec<String>,
    pub duration_ms: Vec<Option<i32>>,
    pub trace_id: Vec<Option<String>>,
    pub operation: Vec<Option<String>>,
    pub status_code: Vec<Option<i32>>,
    pub error_type: Vec<Option<String>>,
    pub error_message: Vec<Option<String>>,
    pub payload_bytes: Vec<Option<i64>>,
    pub metadata: Vec<serde_json::Value>,
}

/// Inserts a whole batch in ONE statement using UNNEST.
///
/// UNNEST turns parallel arrays back into rows server-side, so a batch of 500
/// events is one round trip and one plan rather than 500 of each. This is the
/// standard Postgres bulk-insert shape, and it is why the service transposes
/// row-shaped input into column-shaped arrays first.
pub async fn insert_batch(db: &PgPool, org_id: Uuid, cols: &EventColumns) -> Result<u64, ApiError> {
    let result = sqlx::query!(
        r#"INSERT INTO telemetry_events
             (organization_id, integration_id, environment_id, occurred_at, status,
              duration_ms, trace_id, operation, status_code, error_type,
              error_message, payload_bytes, metadata)
           SELECT $1, u.integration_id, u.environment_id, u.occurred_at, u.status,
                  u.duration_ms, u.trace_id, u.operation, u.status_code, u.error_type,
                  u.error_message, u.payload_bytes, u.metadata
           FROM UNNEST(
                  $2::uuid[], $3::uuid[], $4::timestamptz[], $5::text[], $6::int4[],
                  $7::text[], $8::text[], $9::int4[], $10::text[], $11::text[],
                  $12::int8[], $13::jsonb[]
                ) AS u(integration_id, environment_id, occurred_at, status,
                       duration_ms, trace_id, operation, status_code, error_type,
                       error_message, payload_bytes, metadata)"#,
        org_id,
        &cols.integration_id,
        &cols.environment_id as &[Option<Uuid>],
        &cols.occurred_at,
        &cols.status,
        &cols.duration_ms as &[Option<i32>],
        &cols.trace_id as &[Option<String>],
        &cols.operation as &[Option<String>],
        &cols.status_code as &[Option<i32>],
        &cols.error_type as &[Option<String>],
        &cols.error_message as &[Option<String>],
        &cols.payload_bytes as &[Option<i64>],
        &cols.metadata
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected())
}

/// How many of these integration ids exist in this organization?
///
/// One query rather than one per event: the service compares the count to the
/// number of distinct ids it asked about.
pub async fn count_integrations_in_org(
    db: &PgPool,
    org_id: Uuid,
    ids: &[Uuid],
) -> Result<i64, ApiError> {
    let count = sqlx::query_scalar!(
        r#"SELECT count(*) AS "count!"
           FROM integrations
           WHERE organization_id = $1 AND id = ANY($2::uuid[])"#,
        org_id,
        ids
    )
    .fetch_one(db)
    .await?;

    Ok(count)
}

/// Ensures the monthly partitions around now exist. Called at startup.
pub async fn ensure_partitions(db: &PgPool, months_ahead: i32) -> Result<(), ApiError> {
    for m in -1..=months_ahead {
        sqlx::query!(
            "SELECT ensure_telemetry_partition(now() + ($1 || ' month')::INTERVAL)",
            m.to_string()
        )
        .fetch_one(db)
        .await?;
    }
    Ok(())
}

/// Every filter `list` accepts.
///
/// A struct rather than eight positional arguments: six of them are
/// `Option`s, and two of those borrow strings from the request. The
/// lifetime is there because the string filters are borrowed from the
/// request rather than copied, and the struct may not outlive them.
pub struct ListFilters<'a> {
    pub integration_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub status: Option<&'a str>,
    pub trace_id: Option<&'a str>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: i64,
}

/// Lists events newest-first, with every filter optional.
///
/// Same `($n IS NULL OR ...)` shape used elsewhere, so one static statement
/// serves every combination of filters.
pub async fn list(
    db: &PgPool,
    org_id: Uuid,
    filters: ListFilters<'_>,
) -> Result<Vec<TelemetryEvent>, ApiError> {
    let rows = sqlx::query_as!(
        TelemetryEventRow,
        r#"SELECT id, organization_id, integration_id, environment_id, occurred_at,
                  status, duration_ms, trace_id, operation, status_code,
                  error_type, error_message, payload_bytes, metadata, received_at
           FROM telemetry_events
           WHERE organization_id = $1
             AND ($2::uuid IS NULL OR integration_id = $2)
             AND ($3::uuid IS NULL OR environment_id = $3)
             AND ($4::text IS NULL OR status = $4)
             AND ($5::text IS NULL OR trace_id = $5)
             AND ($6::timestamptz IS NULL OR occurred_at >= $6)
             AND ($7::timestamptz IS NULL OR occurred_at < $7)
           ORDER BY occurred_at DESC
           LIMIT $8"#,
        org_id,
        filters.integration_id,
        filters.environment_id,
        filters.status,
        filters.trace_id,
        filters.from,
        filters.to,
        filters.limit
    )
    .fetch_all(db)
    .await?;

    rows.into_iter()
        .map(TelemetryEventRow::into_domain)
        .collect()
}

/// Aggregates a window in one pass.
///
/// `FILTER (WHERE ...)` counts several conditions in a single scan instead of
/// one query per status. `percentile_cont` interpolates between samples, so a
/// p95 is a real percentile rather than the nearest recorded value.
///
/// Every aggregate here is a computed expression, so Postgres reports it as
/// nullable; `count` never is, hence the `!` on those and Option on the rest.
pub async fn summary(
    db: &PgPool,
    org_id: Uuid,
    integration_id: Option<Uuid>,
    environment_id: Option<Uuid>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
) -> Result<SummaryRow, ApiError> {
    let row = sqlx::query_as!(
        SummaryRow,
        r#"SELECT
             count(*)                                        AS "total!",
             count(*) FILTER (WHERE status = 'SUCCESS')      AS "success!",
             count(*) FILTER (WHERE status = 'FAILURE')      AS "failure!",
             count(*) FILTER (WHERE status = 'TIMEOUT')      AS "timeout!",
             count(*) FILTER (WHERE status = 'REJECTED')     AS "rejected!",
             percentile_cont(0.50) WITHIN GROUP (ORDER BY duration_ms)
                                                             AS "p50_duration_ms",
             percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ms)
                                                             AS "p95_duration_ms",
             percentile_cont(0.99) WITHIN GROUP (ORDER BY duration_ms)
                                                             AS "p99_duration_ms"
           FROM telemetry_events
           WHERE organization_id = $1
             AND ($2::uuid IS NULL OR integration_id = $2)
             AND ($3::uuid IS NULL OR environment_id = $3)
             AND ($4::timestamptz IS NULL OR occurred_at >= $4)
             AND ($5::timestamptz IS NULL OR occurred_at < $5)"#,
        org_id,
        integration_id,
        environment_id,
        from,
        to
    )
    .fetch_one(db)
    .await?;

    Ok(row)
}

/// Buckets telemetry over time for a chart.
///
/// `date_bin` snaps each event to the start of its bucket, using a fixed
/// origin so bucket boundaries are stable between calls: without that, two
/// requests a minute apart would return points that do not line up.
///
/// Buckets with no events are absent rather than zero. Filling gaps is the
/// caller's job, because only the caller knows whether a gap should render as
/// a zero or as a break in the line.
pub async fn series(
    db: &PgPool,
    org_id: Uuid,
    integration_id: Option<Uuid>,
    environment_id: Option<Uuid>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    bucket_seconds: i32,
) -> Result<Vec<SeriesRow>, ApiError> {
    let rows = sqlx::query_as!(
        SeriesRow,
        r#"SELECT
             date_bin(
               make_interval(secs => $6::int),
               occurred_at,
               TIMESTAMPTZ '2000-01-01 00:00:00+00'
             )                                            AS "bucket!",
             count(*)                                     AS "total!",
             count(*) FILTER (WHERE status = 'SUCCESS')   AS "success!",
             count(*) FILTER (WHERE status = 'FAILURE')   AS "failure!",
             count(*) FILTER (WHERE status = 'TIMEOUT')   AS "timeout!",
             count(*) FILTER (WHERE status = 'REJECTED')  AS "rejected!",
             percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ms)
                                                          AS "p95_duration_ms"
           FROM telemetry_events
           WHERE organization_id = $1
             AND ($2::uuid IS NULL OR integration_id = $2)
             AND ($3::uuid IS NULL OR environment_id = $3)
             AND occurred_at >= $4
             AND occurred_at < $5
           GROUP BY 1
           ORDER BY 1"#,
        org_id,
        integration_id,
        environment_id,
        from,
        to,
        bucket_seconds
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}
