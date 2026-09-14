use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use std::collections::HashSet;
use uuid::Uuid;

use crate::api_keys::AuthenticatedKey;
use crate::error::ApiError;

use super::dto::{IngestBatch, ListTelemetryQuery, SeriesQuery, SummaryQuery};
use super::model::{IngestResult, SeriesPoint, TelemetryEvent, TelemetrySummary};
use super::repository::{self, EventColumns};

/// Most requests a single batch may carry. Beyond this the client should split,
/// so one caller cannot hold a connection and a transaction open indefinitely.
const MAX_BATCH: usize = 1_000;
/// How far back an event may be timestamped. Bounded because retention drops
/// old partitions, and because an event older than this is not observability,
/// it is a backfill that would land where no partition exists.
const MAX_BACKDATE_DAYS: i64 = 30;
/// Tolerance for a client whose clock runs fast.
const MAX_CLOCK_SKEW_MINUTES: i64 = 60;

/// Default and maximum page size for the query endpoint.
const DEFAULT_LIMIT: i64 = 100;
const MAX_LIMIT: i64 = 1_000;

/// Accepts a batch of events on behalf of an authenticated key.
///
/// The organization comes from the credential, never from the body: a caller
/// cannot write events into someone else's tenant by naming it.
pub async fn ingest(
    db: &PgPool,
    auth: AuthenticatedKey,
    batch: IngestBatch,
) -> Result<IngestResult, ApiError> {
    if batch.events.is_empty() {
        return Err(ApiError::Validation("events must not be empty".to_string()));
    }
    if batch.events.len() > MAX_BATCH {
        return Err(ApiError::Validation(format!(
            "batch holds {} events; the maximum is {MAX_BATCH}",
            batch.events.len()
        )));
    }

    let now = Utc::now();
    let earliest = now - Duration::days(MAX_BACKDATE_DAYS);
    let latest = now + Duration::minutes(MAX_CLOCK_SKEW_MINUTES);

    // Verify every referenced integration in one query rather than per event.
    let unique: Vec<Uuid> = batch
        .events
        .iter()
        .map(|e| e.integration_id)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    let found = repository::count_integrations_in_org(db, auth.organization_id, &unique).await?;
    if found != unique.len() as i64 {
        return Err(ApiError::Validation(
            "one or more integration_id values do not exist in this organization".to_string(),
        ));
    }

    // Transpose row-shaped input into column-shaped arrays for the bulk insert.
    let mut cols = EventColumns {
        integration_id: Vec::with_capacity(batch.events.len()),
        environment_id: Vec::with_capacity(batch.events.len()),
        occurred_at: Vec::with_capacity(batch.events.len()),
        status: Vec::with_capacity(batch.events.len()),
        duration_ms: Vec::with_capacity(batch.events.len()),
        trace_id: Vec::with_capacity(batch.events.len()),
        operation: Vec::with_capacity(batch.events.len()),
        status_code: Vec::with_capacity(batch.events.len()),
        error_type: Vec::with_capacity(batch.events.len()),
        error_message: Vec::with_capacity(batch.events.len()),
        payload_bytes: Vec::with_capacity(batch.events.len()),
        metadata: Vec::with_capacity(batch.events.len()),
    };

    for (index, event) in batch.events.into_iter().enumerate() {
        let occurred_at = event.occurred_at.unwrap_or(now);
        if occurred_at < earliest || occurred_at > latest {
            return Err(ApiError::Validation(format!(
                "events[{index}].occurred_at is outside the accepted window \
                 (at most {MAX_BACKDATE_DAYS} days old and {MAX_CLOCK_SKEW_MINUTES} minutes ahead)"
            )));
        }

        if let Some(duration) = event.duration_ms {
            if duration < 0 {
                return Err(ApiError::Validation(format!(
                    "events[{index}].duration_ms must not be negative"
                )));
            }
        }

        // A key pinned to an environment defines the environment. An event
        // naming a different one is a misconfigured agent, not a valid write.
        let environment_id = match (auth.environment_id, event.environment_id) {
            (Some(pinned), Some(claimed)) if pinned != claimed => {
                return Err(ApiError::Validation(format!(
                    "events[{index}].environment_id does not match the environment \
                     this API key is pinned to"
                )));
            }
            (Some(pinned), _) => Some(pinned),
            (None, claimed) => claimed,
        };

        cols.integration_id.push(event.integration_id);
        cols.environment_id.push(environment_id);
        cols.occurred_at.push(occurred_at);
        cols.status.push(event.status.as_str().to_string());
        cols.duration_ms.push(event.duration_ms);
        cols.trace_id.push(event.trace_id);
        cols.operation.push(event.operation);
        cols.status_code.push(event.status_code);
        cols.error_type.push(event.error_type);
        cols.error_message.push(event.error_message);
        cols.payload_bytes.push(event.payload_bytes);
        cols.metadata.push(if event.metadata.is_null() {
            serde_json::json!({})
        } else {
            event.metadata
        });
    }

    let accepted = repository::insert_batch(db, auth.organization_id, &cols).await?;
    Ok(IngestResult {
        accepted: accepted as usize,
    })
}

pub async fn list(
    db: &PgPool,
    org_id: Uuid,
    query: ListTelemetryQuery,
) -> Result<Vec<TelemetryEvent>, ApiError> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    validate_window(query.from, query.to)?;

    repository::list(
        db,
        org_id,
        repository::ListFilters {
            integration_id: query.integration_id,
            environment_id: query.environment_id,
            status: query.status.map(|s| s.as_str()),
            trace_id: query.trace_id.as_deref(),
            from: query.from,
            to: query.to,
            limit,
        },
    )
    .await
}

pub async fn summary(
    db: &PgPool,
    org_id: Uuid,
    query: SummaryQuery,
) -> Result<TelemetrySummary, ApiError> {
    validate_window(query.from, query.to)?;

    let row = repository::summary(
        db,
        org_id,
        query.integration_id,
        query.environment_id,
        query.from,
        query.to,
    )
    .await?;

    Ok(row.into_domain())
}

fn validate_window(from: Option<DateTime<Utc>>, to: Option<DateTime<Utc>>) -> Result<(), ApiError> {
    if let (Some(from), Some(to)) = (from, to) {
        if to <= from {
            return Err(ApiError::Validation("to must be after from".to_string()));
        }
    }
    Ok(())
}

/// How many points a chart should get when the caller does not choose.
const TARGET_POINTS: i64 = 60;
/// Hard ceiling, so a one-second bucket over a month cannot ask Postgres for
/// millions of rows or hand the browser a series it cannot draw.
const MAX_POINTS: i64 = 1_000;
const MIN_BUCKET_SECONDS: i64 = 10;

pub async fn series(
    db: &PgPool,
    org_id: Uuid,
    query: SeriesQuery,
) -> Result<Vec<SeriesPoint>, ApiError> {
    let to = query.to.unwrap_or_else(Utc::now);
    let from = query.from.unwrap_or(to - Duration::hours(24));

    if to <= from {
        return Err(ApiError::Validation("to must be after from".to_string()));
    }

    let span_seconds = (to - from).num_seconds().max(1);

    // Pick a bucket that yields a readable number of points, then widen it if
    // the caller asked for something that would produce too many.
    let requested = query
        .bucket_seconds
        .unwrap_or_else(|| (span_seconds / TARGET_POINTS).max(MIN_BUCKET_SECONDS));
    let smallest_allowed = (span_seconds / MAX_POINTS).max(MIN_BUCKET_SECONDS);
    let bucket_seconds = requested.max(smallest_allowed);

    let rows = repository::series(
        db,
        org_id,
        query.integration_id,
        query.environment_id,
        from,
        to,
        bucket_seconds as i32,
    )
    .await?;

    Ok(rows.into_iter().map(|r| r.into_domain()).collect())
}
