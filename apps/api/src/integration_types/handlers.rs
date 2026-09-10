use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::auth::OrgContext;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path};
use crate::AppState;

use super::dto::CreateIntegrationType;
use super::model::IntegrationType;
use super::service;

#[utoipa::path(
    get,
    path = "/api/v1/integration-types",
    tag = "Reference data",
    responses((status = 200,
               description = "The built-in catalogue plus this organization's own types",
               body = Vec<IntegrationType>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: OrgContext,
) -> Result<Json<Vec<IntegrationType>>, ApiError> {
    Ok(Json(service::list(&state.db, ctx.organization_id).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/integration-types",
    tag = "Reference data",
    request_body = CreateIntegrationType,
    responses(
        (status = 201, description = "Created, visible only to this organization",
         body = IntegrationType),
        (status = 403, description = "Requires EDITOR", body = ErrorBody),
        (status = 409, description = "That key is already used here", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: OrgContext,
    Json(body): Json<CreateIntegrationType>,
) -> Result<(StatusCode, Json<IntegrationType>), ApiError> {
    ctx.require_write()?;
    let created = service::create(&state.db, ctx.organization_id, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/integration-types/{id}",
    tag = "Reference data",
    params(("id" = Uuid, Path, description = "Integration type id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "No such custom type here; built-ins are permanent",
         body = ErrorBody),
        (status = 409, description = "Integrations still use it", body = ErrorBody),
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
