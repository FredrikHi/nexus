use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::auth::OrgContext;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path, Query};
use crate::AppState;

use super::dto::{CreateComponent, ListComponentsQuery, UpdateComponent};
use super::model::Component;
use super::service;

// Each handler: extract inputs, call the service, wrap the result for HTTP.
// `?` turns any ApiError into the right HTTP response via IntoResponse.
// Body extractors (Json) must come LAST in the argument list.

#[utoipa::path(
    get,
    path = "/api/v1/components",
    tag = "Components",
    params(ListComponentsQuery),
    responses(
        (status = 200, description = "Matching components", body = Vec<Component>),
        (status = 400, description = "Malformed query parameter", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: OrgContext,
    Query(params): Query<ListComponentsQuery>,
) -> Result<Json<Vec<Component>>, ApiError> {
    Ok(Json(
        service::list(&state.db, ctx.organization_id, params.system_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/components/{id}",
    tag = "Components",
    params(("id" = Uuid, Path, description = "Component id")),
    responses(
        (status = 200, description = "The component", body = Component),
        (status = 400, description = "Malformed id", body = ErrorBody),
        (status = 404, description = "No such component", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Component>, ApiError> {
    Ok(Json(
        service::get(&state.db, ctx.organization_id, id).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/components",
    tag = "Components",
    request_body = CreateComponent,
    responses(
        (status = 201, description = "Created", body = Component),
        (status = 400, description = "Body was not valid JSON", body = ErrorBody),
        (status = 409, description = "Slug already taken within the system", body = ErrorBody),
        (status = 415, description = "Missing or wrong Content-Type", body = ErrorBody),
        (status = 422, description = "Invalid values, or system_id does not exist", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: OrgContext,
    Json(body): Json<CreateComponent>,
) -> Result<(StatusCode, Json<Component>), ApiError> {
    ctx.require_write()?;
    let created = service::create(&state.db, ctx.organization_id, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    patch,
    path = "/api/v1/components/{id}",
    tag = "Components",
    params(("id" = Uuid, Path, description = "Component id")),
    request_body = UpdateComponent,
    responses(
        (status = 200, description = "Updated", body = Component),
        (status = 400, description = "Malformed id or body", body = ErrorBody),
        (status = 404, description = "No such component", body = ErrorBody),
        (status = 409, description = "Slug already taken within the system", body = ErrorBody),
        (status = 422, description = "Body parsed but the values are invalid", body = ErrorBody),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateComponent>,
) -> Result<Json<Component>, ApiError> {
    ctx.require_write()?;
    Ok(Json(
        service::update(&state.db, ctx.organization_id, id, body).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/components/{id}",
    tag = "Components",
    params(("id" = Uuid, Path, description = "Component id")),
    responses(
        (status = 204, description = "Deleted; integrations touching it cascade"),
        (status = 400, description = "Malformed id", body = ErrorBody),
        (status = 404, description = "No such component", body = ErrorBody),
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
