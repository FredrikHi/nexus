use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::auth::OrgContext;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path};
use crate::AppState;

use super::dto::CreateApiKey;
use super::model::{ApiKey, CreatedApiKey};
use super::service;

#[utoipa::path(
    get,
    path = "/api/v1/api-keys",
    tag = "API keys",
    responses(
        (status = 200, description = "Keys in the organization. Tokens are never included.",
         body = Vec<ApiKey>),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: OrgContext,
) -> Result<Json<Vec<ApiKey>>, ApiError> {
    // Keys can ingest on the organization's behalf, so listing them is an
    // administrative act rather than an ordinary read.
    ctx.require_admin()?;
    Ok(Json(service::list(&state.db, ctx.organization_id).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/api-keys",
    tag = "API keys",
    request_body = CreateApiKey,
    responses(
        (status = 201,
         description = "Created. The token field is shown ONCE and cannot be retrieved again.",
         body = CreatedApiKey),
        (status = 409, description = "A key with that name already exists", body = ErrorBody),
        (status = 422, description = "Invalid values", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: OrgContext,
    Json(body): Json<CreateApiKey>,
) -> Result<(StatusCode, Json<CreatedApiKey>), ApiError> {
    ctx.require_admin()?;
    let created = service::create(&state.db, ctx.organization_id, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/api-keys/{id}",
    tag = "API keys",
    params(("id" = Uuid, Path, description = "API key id")),
    responses(
        (status = 200,
         description = "Revoked. The key stops working immediately; the row is kept as an audit \
                        record of what existed and when it was withdrawn.",
         body = ApiKey),
        (status = 404, description = "No such active key", body = ErrorBody),
    )
)]
pub async fn revoke(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiKey>, ApiError> {
    ctx.require_admin()?;
    Ok(Json(service::revoke(&state.db, ctx.organization_id, id).await?))
}
