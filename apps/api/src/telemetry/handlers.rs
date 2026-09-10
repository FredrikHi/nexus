use axum::extract::State;
use axum::http::StatusCode;

use crate::api_keys::ApiKeyAuth;
use crate::auth::OrgContext;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Query};
use crate::AppState;

use super::dto::{IngestBatch, ListTelemetryQuery, SeriesQuery, SummaryQuery};
use super::model::{IngestResult, SeriesPoint, TelemetryEvent, TelemetrySummary};
use super::service;

#[utoipa::path(
    post,
    path = "/api/v1/telemetry",
    tag = "Telemetry",
    request_body = IngestBatch,
    security(("api_key" = [])),
    responses(
        (status = 202, description = "Batch accepted", body = IngestResult),
        (status = 401, description = "Missing, unknown, revoked or expired API key", body = ErrorBody),
        (status = 422,
         description = "Empty or oversized batch, an unknown integration, a timestamp outside \
                        the accepted window, or an environment the key is not allowed to write",
         body = ErrorBody),
    )
)]
pub async fn ingest(
    State(state): State<AppState>,
    ApiKeyAuth(auth): ApiKeyAuth,
    Json(body): Json<IngestBatch>,
) -> Result<(StatusCode, Json<IngestResult>), ApiError> {
    let result = service::ingest(&state.db, auth, body).await?;
    // 202: the events are durably stored, but nothing downstream has derived
    // health or incidents from them yet.
    Ok((StatusCode::ACCEPTED, Json(result)))
}

#[utoipa::path(
    get,
    path = "/api/v1/telemetry",
    tag = "Telemetry",
    params(ListTelemetryQuery),
    responses(
        (status = 200, description = "Matching events, newest first", body = Vec<TelemetryEvent>),
        (status = 400, description = "Malformed query parameter", body = ErrorBody),
        (status = 422, description = "Inverted time window", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: OrgContext,
    Query(params): Query<ListTelemetryQuery>,
) -> Result<Json<Vec<TelemetryEvent>>, ApiError> {
    Ok(Json(service::list(&state.db, ctx.organization_id, params).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/telemetry/summary",
    tag = "Telemetry",
    params(SummaryQuery),
    responses(
        (status = 200, description = "Counts, error rate and duration percentiles",
         body = TelemetrySummary),
        (status = 400, description = "Malformed query parameter", body = ErrorBody),
        (status = 422, description = "Inverted time window", body = ErrorBody),
    )
)]
pub async fn summary(
    State(state): State<AppState>,
    ctx: OrgContext,
    Query(params): Query<SummaryQuery>,
) -> Result<Json<TelemetrySummary>, ApiError> {
    Ok(Json(service::summary(&state.db, ctx.organization_id, params).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/telemetry/series",
    tag = "Telemetry",
    params(SeriesQuery),
    responses(
        (status = 200,
         description = "Telemetry bucketed over time. Buckets with no events are omitted                         rather than returned as zero.",
         body = Vec<SeriesPoint>),
        (status = 422, description = "Inverted time window", body = ErrorBody),
    )
)]
pub async fn series(
    State(state): State<AppState>,
    ctx: OrgContext,
    Query(params): Query<SeriesQuery>,
) -> Result<Json<Vec<SeriesPoint>>, ApiError> {
    Ok(Json(service::series(&state.db, ctx.organization_id, params).await?))
}
