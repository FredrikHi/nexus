use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
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

    // Each event names its integration by slug or by id. Both kinds are
    // resolved up front, so a 500-event batch costs two lookups rather than
    // five hundred.
    let mut slugs: HashSet<String> = HashSet::new();
    let mut ids: HashSet<Uuid> = HashSet::new();

    for (index, event) in batch.events.iter().enumerate() {
        match (event.integration.as_deref(), event.integration_id) {
            (Some(slug), None) => {
                slugs.insert(slug.to_string());
            }
            (None, Some(id)) => {
                ids.insert(id);
            }
            (Some(_), Some(_)) => {
                return Err(ApiError::Validation(format!(
                    "events[{index}] sets both integration and integration_id; use one"
                )))
            }
            (None, None) => {
                return Err(ApiError::Validation(format!(
                    "events[{index}] must set integration (the slug) or integration_id"
                )))
            }
        }
    }

    let ids: Vec<Uuid> = ids.into_iter().collect();
    let known_ids = repository::existing_integration_ids(db, auth.organization_id, &ids).await?;
    // Naming what was not found turns a support question into a fix. The old
    // message said only that something in the batch was wrong.
    let unknown: Vec<String> = ids
        .iter()
        .filter(|id| !known_ids.contains(id))
        .map(|id| id.to_string())
        .collect();
    if !unknown.is_empty() {
        return Err(ApiError::Validation(format!(
            "no integration in this organization has id: {}",
            unknown.join(", ")
        )));
    }

    let wanted: Vec<String> = slugs.into_iter().collect();
    let mut by_slug = repository::resolve_slugs(db, auth.organization_id, &wanted).await?;

    let unknown: Vec<String> = wanted
        .iter()
        .filter(|slug| !by_slug.contains_key(*slug))
        .cloned()
        .collect();

    if !unknown.is_empty() {
        if !auth.allow_auto_create {
            return Err(ApiError::Validation(format!(
                "no integration in this organization has slug: {}",
                unknown.join(", ")
            )));
        }
        // This key is allowed to describe its own landscape, so a slug nobody
        // has modelled yet becomes an integration rather than a refusal. It
        // lands under the Unmapped system for someone to wire up later.
        let created = repository::ensure_integrations(db, auth.organization_id, &unknown).await?;
        by_slug.extend(created);
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

        let integration_id = match (event.integration.as_deref(), event.integration_id) {
            (Some(slug), _) => *by_slug.get(slug).ok_or_else(|| {
                ApiError::Internal(format!(
                    "slug '{slug}' resolved before the loop but not inside it"
                ))
            })?,
            (None, Some(id)) => id,
            (None, None) => {
                return Err(ApiError::Validation(format!(
                    "events[{index}] must name an integration"
                )))
            }
        };

        cols.integration_id.push(integration_id);
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

    let measured: Vec<SeriesPoint> = rows.into_iter().map(|r| r.into_domain()).collect();
    Ok(fill_gaps(measured, from, to, bucket_seconds))
}

/// Puts an empty bucket wherever nothing happened.
///
/// The query returns only buckets that contain events, and a chart plots what
/// it is given at even spacing. A week with traffic on two days therefore
/// renders as two adjacent columns rather than a week that was mostly quiet,
/// which makes every window past a day look much the same. Filling the gaps
/// makes the axis mean what it appears to mean, and makes silence visible
/// rather than absent.
fn fill_gaps(
    measured: Vec<SeriesPoint>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    bucket_seconds: i64,
) -> Vec<SeriesPoint> {
    // date_bin aligns buckets to this origin, so filled buckets must align to
    // it too, or they would sit between the real ones instead of beside them.
    let Some(origin) = DateTime::<Utc>::from_timestamp(946_684_800, 0) else {
        return measured;
    };

    let by_bucket: HashMap<DateTime<Utc>, SeriesPoint> =
        measured.into_iter().map(|p| (p.bucket, p)).collect();

    // Floor the start onto a bucket boundary. div_euclid rather than plain
    // division, so a timestamp before the origin still rounds downwards.
    let offset = (from - origin).num_seconds().div_euclid(bucket_seconds);
    let mut cursor = origin + Duration::seconds(offset * bucket_seconds);

    let mut filled = Vec::with_capacity(by_bucket.len() + 16);
    while cursor < to {
        filled.push(by_bucket.get(&cursor).cloned().unwrap_or(SeriesPoint {
            bucket: cursor,
            total: 0,
            success: 0,
            failure: 0,
            timeout: 0,
            rejected: 0,
            error_rate: 0.0,
            p95_duration_ms: None,
        }));
        cursor += Duration::seconds(bucket_seconds);
    }

    filled
}
