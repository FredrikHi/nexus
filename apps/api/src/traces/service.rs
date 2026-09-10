use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::dto::ListTracesQuery;
use super::model::{TraceDetail, TraceSummary};
use super::repository;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 500;

pub async fn list(
    db: &PgPool,
    org_id: Uuid,
    query: ListTracesQuery,
) -> Result<Vec<TraceSummary>, ApiError> {
    if let (Some(from), Some(to)) = (query.from, query.to) {
        if to <= from {
            return Err(ApiError::Validation("to must be after from".to_string()));
        }
    }
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    let rows = repository::list(
        db,
        org_id,
        query.integration_id,
        query.from,
        query.to,
        query.only_errors.unwrap_or(false),
        limit,
    )
    .await?;

    rows.into_iter().map(|r| r.into_domain()).collect()
}

/// One trace with its spans in the order they happened.
///
/// The summary is fetched first so an unknown trace is a clean 404 rather than
/// an empty span list, which would be indistinguishable from a trace whose
/// events have aged out of retention.
pub async fn get(
    db: &PgPool,
    org_id: Uuid,
    trace_id: &str,
) -> Result<TraceDetail, ApiError> {
    if trace_id.trim().is_empty() {
        return Err(ApiError::Validation("trace_id must not be empty".to_string()));
    }
    let summary = repository::summary(db, org_id, trace_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("trace {trace_id} not found")))?
        .into_domain()?;

    let spans = repository::spans(db, org_id, trace_id).await?;

    Ok(TraceDetail { summary, spans })
}
