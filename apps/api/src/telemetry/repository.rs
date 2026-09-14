use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
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

/// Which of these integration ids exist in this organization.
///
/// Returns the ones found rather than a count, so the caller can name the ones
/// that are missing instead of saying that something, somewhere, is wrong.
pub async fn existing_integration_ids(
    db: &PgPool,
    org_id: Uuid,
    ids: &[Uuid],
) -> Result<HashSet<Uuid>, ApiError> {
    let rows = sqlx::query_scalar!(
        r#"SELECT id FROM integrations
           WHERE organization_id = $1 AND id = ANY($2::uuid[])"#,
        org_id,
        ids
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().collect())
}

/// Maps integration slugs to their ids, omitting any that do not exist.
///
/// One query for the whole batch: an ingest of 500 events referencing six
/// integrations should cost one lookup, not five hundred.
pub async fn resolve_slugs(
    db: &PgPool,
    org_id: Uuid,
    slugs: &[String],
) -> Result<HashMap<String, Uuid>, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT id, slug FROM integrations
           WHERE organization_id = $1 AND slug = ANY($2::text[])"#,
        org_id,
        slugs
    )
    .fetch_all(db)
    .await?;

    Ok(rows.into_iter().map(|row| (row.slug, row.id)).collect())
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

/// Creates any of these slugs that do not exist yet, and returns the full map.
///
/// An auto-created integration still has to point somewhere, because both
/// endpoints are NOT NULL. It points at a placeholder pair under an "Unmapped"
/// system until someone wires it up, which keeps the graph honest: the call is
/// known to happen, what it runs between is not.
///
/// Everything is `ON CONFLICT DO NOTHING` inside one transaction, so two
/// workers ingesting the same new slug at the same time produce one row rather
/// than a unique-violation for the loser.
pub async fn ensure_integrations(
    db: &PgPool,
    org_id: Uuid,
    slugs: &[String],
) -> Result<HashMap<String, Uuid>, ApiError> {
    let mut tx = db.begin().await?;

    // `DO UPDATE SET slug = EXCLUDED.slug` is a no-op write whose only purpose
    // is to make RETURNING produce a row on conflict as well as on insert.
    let system = sqlx::query_scalar!(
        r#"INSERT INTO systems (organization_id, name, slug, system_type, description)
           VALUES ($1, 'Unmapped', 'unmapped', 'OTHER',
                   'Integrations discovered from telemetry that nobody has wired up yet.')
           ON CONFLICT (organization_id, slug) DO UPDATE SET slug = EXCLUDED.slug
           RETURNING id"#,
        org_id
    )
    .fetch_one(&mut *tx)
    .await?;

    let mut endpoints = Vec::with_capacity(2);
    for (name, slug) in [("Caller", "caller"), ("Dependency", "dependency")] {
        let id = sqlx::query_scalar!(
            r#"INSERT INTO components (system_id, name, slug, component_type)
               VALUES ($1, $2, $3, 'OTHER')
               ON CONFLICT (system_id, slug) DO UPDATE SET slug = EXCLUDED.slug
               RETURNING id"#,
            system,
            name,
            slug
        )
        .fetch_one(&mut *tx)
        .await?;
        endpoints.push(id);
    }

    // A built-in type, so it is shared rather than owned by this organization.
    let integration_type = sqlx::query_scalar!(
        r#"SELECT id FROM integration_types
           WHERE key = 'HTTP' AND organization_id IS NULL"#
    )
    .fetch_one(&mut *tx)
    .await?;

    // The name is the slug on purpose. It reads as unfinished, which it is,
    // and renaming it in the UI leaves the slug alone so the code keeps working.
    sqlx::query!(
        r#"INSERT INTO integrations
             (organization_id, name, slug, source_component_id,
              destination_component_id, integration_type_id, metadata)
           SELECT $1, slug, slug, $3, $4, $5, '{"discovered": true}'::jsonb
           FROM unnest($2::text[]) AS slug
           ON CONFLICT (organization_id, slug) DO NOTHING"#,
        org_id,
        slugs,
        endpoints[0],
        endpoints[1],
        integration_type
    )
    .execute(&mut *tx)
    .await?;

    let rows = sqlx::query!(
        r#"SELECT id, slug FROM integrations
           WHERE organization_id = $1 AND slug = ANY($2::text[])"#,
        org_id,
        slugs
    )
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(rows.into_iter().map(|row| (row.slug, row.id)).collect())
}
