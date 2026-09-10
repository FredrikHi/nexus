use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

use crate::auth::OrgContext;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path, Query};
use crate::AppState;

use super::dto::{CreateEnvironment, UpdateEnvironment};
use super::model::Environment;
use super::service;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DeleteQuery {
    /// Detach any integrations still using this environment.
    pub force: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/v1/environments",
    tag = "Reference data",
    responses((status = 200, description = "Environments in the active organization",
               body = Vec<Environment>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: OrgContext,
) -> Result<Json<Vec<Environment>>, ApiError> {
    Ok(Json(service::list(&state.db, ctx.organization_id).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/environments",
    tag = "Reference data",
    request_body = CreateEnvironment,
    responses(
        (status = 201, description = "Created", body = Environment),
        (status = 403, description = "Requires EDITOR", body = ErrorBody),
        (status = 409, description = "That slug is already used here", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: OrgContext,
    Json(body): Json<CreateEnvironment>,
) -> Result<(StatusCode, Json<Environment>), ApiError> {
    ctx.require_write()?;
    let created = service::create(&state.db, ctx.organization_id, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/environments/{id}",
    tag = "Reference data",
    params(("id" = Uuid, Path, description = "Environment id")),
    request_body = UpdateEnvironment,
    responses(
        (status = 200, description = "Updated", body = Environment),
        (status = 404, description = "No such environment", body = ErrorBody),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateEnvironment>,
) -> Result<Json<Environment>, ApiError> {
    ctx.require_write()?;
    Ok(Json(service::update(&state.db, ctx.organization_id, id, body).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/environments/{id}",
    tag = "Reference data",
    params(("id" = Uuid, Path, description = "Environment id"), DeleteQuery),
    responses(
        (status = 204, description = "Deleted"),
        (status = 409,
         description = "Integrations still use it. Repeat with force=true to detach them.",
         body = ErrorBody),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
    Query(params): Query<DeleteQuery>,
) -> Result<StatusCode, ApiError> {
    ctx.require_write()?;
    service::delete(&state.db, &ctx, id, params.force.unwrap_or(false)).await?;
    Ok(StatusCode::NO_CONTENT)
}
