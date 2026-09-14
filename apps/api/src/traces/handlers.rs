use axum::extract::State;

use crate::auth::OrgContext;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path, Query};
use crate::AppState;

use super::dto::ListTracesQuery;
use super::model::{TraceDetail, TraceSummary};
use super::service;

#[utoipa::path(
    get,
    path = "/api/v1/traces",
    tag = "Traces",
    params(ListTracesQuery),
    responses(
        (status = 200, description = "Trace summaries, newest first", body = Vec<TraceSummary>),
        (status = 400, description = "Malformed query parameter", body = ErrorBody),
        (status = 422, description = "Inverted time window", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: OrgContext,
    Query(params): Query<ListTracesQuery>,
) -> Result<Json<Vec<TraceSummary>>, ApiError> {
    Ok(Json(
        service::list(&state.db, ctx.organization_id, params).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/traces/{trace_id}",
    tag = "Traces",
    params(("trace_id" = String, Path, description = "Correlation id carried by the spans")),
    responses(
        (status = 200, description = "The trace and its spans, oldest first", body = TraceDetail),
        (status = 404, description = "No such trace, or its events have aged out", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(trace_id): Path<String>,
) -> Result<Json<TraceDetail>, ApiError> {
    Ok(Json(
        service::get(&state.db, ctx.organization_id, &trace_id).await?,
    ))
}
