use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

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
pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<ApiKey>>, ApiError> {
    Ok(Json(service::list(&state.db).await?))
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
    Json(body): Json<CreateApiKey>,
) -> Result<(StatusCode, Json<CreatedApiKey>), ApiError> {
    let created = service::create(&state.db, body).await?;
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
    Path(id): Path<Uuid>,
) -> Result<Json<ApiKey>, ApiError> {
    Ok(Json(service::revoke(&state.db, id).await?))
}
