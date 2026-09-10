use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::auth::OrgContext;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path, Query};
use crate::AppState;

use super::dto::{CreateIntegration, ListIntegrationsQuery, UpdateIntegration};
use super::model::Integration;
use super::service;

#[utoipa::path(
    get,
    path = "/api/v1/integrations",
    tag = "Integrations",
    params(ListIntegrationsQuery),
    responses(
        (status = 200, description = "Matching integrations", body = Vec<Integration>),
        (status = 400, description = "Malformed query parameter", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: OrgContext,
    Query(params): Query<ListIntegrationsQuery>,
) -> Result<Json<Vec<Integration>>, ApiError> {
    Ok(Json(
        service::list(
            &state.db,
            ctx.organization_id,
            params.environment_id,
            params.component_id,
        )
        .await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/integrations/{id}",
    tag = "Integrations",
    params(("id" = Uuid, Path, description = "Integration id")),
    responses(
        (status = 200, description = "The integration", body = Integration),
        (status = 400, description = "Malformed id", body = ErrorBody),
        (status = 404, description = "No such integration", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Integration>, ApiError> {
    Ok(Json(service::get(&state.db, ctx.organization_id, id).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/integrations",
    tag = "Integrations",
    request_body = CreateIntegration,
    responses(
        (status = 201, description = "Created", body = Integration),
        (status = 400, description = "Body was not valid JSON", body = ErrorBody),
        (status = 409, description = "Slug already taken in this organization", body = ErrorBody),
        (status = 415, description = "Missing or wrong Content-Type", body = ErrorBody),
        (status = 422,
         description = "Invalid values, an unknown referenced id, or source equal to destination. \
                        Every offending reference is listed in one message.",
         body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: OrgContext,
    Json(body): Json<CreateIntegration>,
) -> Result<(StatusCode, Json<Integration>), ApiError> {
    ctx.require_write()?;
    let created = service::create(&state.db, ctx.organization_id, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/integrations/{id}",
    tag = "Integrations",
    params(("id" = Uuid, Path, description = "Integration id")),
    request_body = UpdateIntegration,
    responses(
        (status = 200, description = "Updated", body = Integration),
        (status = 400, description = "Malformed id or body", body = ErrorBody),
        (status = 404, description = "No such integration", body = ErrorBody),
        (status = 409, description = "Slug already taken in this organization", body = ErrorBody),
        (status = 422,
         description = "Invalid values, or the merged result of the patch is not a valid edge",
         body = ErrorBody),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateIntegration>,
) -> Result<Json<Integration>, ApiError> {
    ctx.require_write()?;
    Ok(Json(service::update(&state.db, ctx.organization_id, id, body).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/integrations/{id}",
    tag = "Integrations",
    params(("id" = Uuid, Path, description = "Integration id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 400, description = "Malformed id", body = ErrorBody),
        (status = 404, description = "No such integration", body = ErrorBody),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ctx.require_write()?;
    service::delete(&state.db, ctx.organization_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
