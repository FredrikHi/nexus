use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::auth::OrgContext;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path};
use crate::AppState;

use super::dto::{CreateSystem, UpdateSystem};
use super::model::System;
use super::service;

// Each handler: extract inputs, call the service, wrap the result for HTTP.
// The `?` turns any ApiError into the right HTTP response (via IntoResponse).
// Note: body extractors (Json) must come LAST in the argument list.
//
// The `#[utoipa::path]` attribute documents the endpoint. It generates a
// hidden type that `super::openapi()` collects; it does not alter the handler.
// Path parameters are declared explicitly because the extractors are our own
// wrappers, which utoipa's axum inference does not recognise.

#[utoipa::path(
    get,
    path = "/api/v1/systems",
    tag = "Systems",
    responses(
        (status = 200, description = "All systems in the organization", body = Vec<System>),
        (status = 500, description = "Internal error", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: OrgContext,
) -> Result<Json<Vec<System>>, ApiError> {
    Ok(Json(service::list(&state.db, ctx.organization_id).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/systems/{id}",
    tag = "Systems",
    params(("id" = Uuid, Path, description = "System id")),
    responses(
        (status = 200, description = "The system", body = System),
        (status = 400, description = "Malformed id", body = ErrorBody),
        (status = 404, description = "No such system", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
) -> Result<Json<System>, ApiError> {
    Ok(Json(
        service::get(&state.db, ctx.organization_id, id).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/systems",
    tag = "Systems",
    request_body = CreateSystem,
    responses(
        (status = 201, description = "Created", body = System),
        (status = 400, description = "Body was not valid JSON", body = ErrorBody),
        (status = 409, description = "Slug already taken in this organization", body = ErrorBody),
        (status = 415, description = "Missing or wrong Content-Type", body = ErrorBody),
        (status = 422, description = "Body parsed but the values are invalid", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: OrgContext,
    Json(body): Json<CreateSystem>,
) -> Result<(StatusCode, Json<System>), ApiError> {
    ctx.require_write()?;
    let created = service::create(&state.db, ctx.organization_id, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/systems/{id}",
    tag = "Systems",
    params(("id" = Uuid, Path, description = "System id")),
    request_body = UpdateSystem,
    responses(
        (status = 200, description = "Updated", body = System),
        (status = 400, description = "Malformed id or body", body = ErrorBody),
        (status = 404, description = "No such system", body = ErrorBody),
        (status = 409, description = "Slug already taken in this organization", body = ErrorBody),
        (status = 422, description = "Body parsed but the values are invalid", body = ErrorBody),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSystem>,
) -> Result<Json<System>, ApiError> {
    ctx.require_write()?;
    Ok(Json(
        service::update(&state.db, ctx.organization_id, id, body).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/systems/{id}",
    tag = "Systems",
    params(("id" = Uuid, Path, description = "System id")),
    responses(
        (status = 204, description = "Deleted; its components cascade"),
        (status = 400, description = "Malformed id", body = ErrorBody),
        (status = 404, description = "No such system", body = ErrorBody),
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
