use std::collections::HashMap;

use axum::extract::State;
use uuid::Uuid;

use crate::auth::OrgContext;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path, Query};
use crate::AppState;

use super::dto::TransitionsQuery;
use super::model::{EvaluationResult, HealthTransition, IntegrationHealth};
use super::service;

#[utoipa::path(
    get,
    path = "/api/v1/integration-health",
    tag = "Integration health",
    responses(
        (status = 200,
         description = "Current health of every monitored integration, worst first",
         body = Vec<IntegrationHealth>),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: OrgContext,
) -> Result<Json<Vec<IntegrationHealth>>, ApiError> {
    Ok(Json(service::list(&state.db, ctx.organization_id).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/integration-health/overview",
    tag = "Integration health",
    responses(
        (status = 200, description = "How many integrations sit in each status",
         body = HashMap<String, usize>),
    )
)]
pub async fn overview(
    State(state): State<AppState>,
    ctx: OrgContext,
) -> Result<Json<HashMap<&'static str, usize>>, ApiError> {
    Ok(Json(service::overview(&state.db, ctx.organization_id).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/integration-health/{integration_id}",
    tag = "Integration health",
    params(("integration_id" = Uuid, Path, description = "Integration id")),
    responses(
        (status = 200, description = "Current health", body = IntegrationHealth),
        (status = 404,
         description = "No health record yet: new, unmonitored, or not evaluated since creation",
         body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(integration_id): Path<Uuid>,
) -> Result<Json<IntegrationHealth>, ApiError> {
    Ok(Json(service::get(&state.db, ctx.organization_id, integration_id).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/integration-health/{integration_id}/transitions",
    tag = "Integration health",
    params(
        ("integration_id" = Uuid, Path, description = "Integration id"),
        TransitionsQuery,
    ),
    responses(
        (status = 200, description = "Status changes, newest first", body = Vec<HealthTransition>),
    )
)]
pub async fn transitions(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(integration_id): Path<Uuid>,
    Query(params): Query<TransitionsQuery>,
) -> Result<Json<Vec<HealthTransition>>, ApiError> {
    Ok(Json(
        service::transitions(&state.db, ctx.organization_id, integration_id, params.limit)
            .await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/integration-health/evaluate",
    tag = "Integration health",
    responses(
        (status = 200, description = "Evaluation ran; reports how many statuses changed",
         body = EvaluationResult),
    )
)]
pub async fn evaluate(
    State(state): State<AppState>,
    ctx: OrgContext,
) -> Result<Json<EvaluationResult>, ApiError> {
    // The same pass the worker runs, but scoped to the caller's own tenant:
    // a refresh button must not evaluate somebody else's organization.
    ctx.require_write()?;
    Ok(Json(service::evaluate(&state.db, ctx.organization_id).await?))
}
