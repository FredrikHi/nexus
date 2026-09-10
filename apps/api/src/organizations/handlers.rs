use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::auth::{Authenticated, OrgContext};
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path};
use crate::AppState;

use super::dto::{AddMember, CreateOrganization, UpdateMemberRole};
use super::model::{Me, Member, Organization};
use super::service;

#[utoipa::path(
    get,
    path = "/api/v1/me",
    tag = "Account",
    responses(
        (status = 200, description = "The caller and every organization they belong to", body = Me),
        (status = 401, description = "Missing or invalid token", body = ErrorBody),
    )
)]
pub async fn me(
    State(state): State<AppState>,
    Authenticated(identity): Authenticated,
) -> Result<Json<Me>, ApiError> {
    Ok(Json(service::me(&state.db, identity).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/organizations",
    tag = "Account",
    request_body = CreateOrganization,
    responses(
        (status = 201, description = "Created; the caller becomes its owner", body = Organization),
        (status = 409, description = "That slug is already taken", body = ErrorBody),
        (status = 422, description = "Invalid values", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    Authenticated(identity): Authenticated,
    Json(body): Json<CreateOrganization>,
) -> Result<(StatusCode, Json<Organization>), ApiError> {
    let created = service::create(&state.db, &identity, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    get,
    path = "/api/v1/organization",
    tag = "Account",
    responses(
        (status = 200, description = "The organization this request is acting in",
         body = Organization),
        (status = 403, description = "Not a member of that organization", body = ErrorBody),
    )
)]
pub async fn current(
    State(state): State<AppState>,
    ctx: OrgContext,
) -> Result<Json<Organization>, ApiError> {
    Ok(Json(service::get(&state.db, &ctx).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/organization/members",
    tag = "Account",
    responses(
        (status = 200, description = "Members of the active organization", body = Vec<Member>),
        (status = 403, description = "Not a member", body = ErrorBody),
    )
)]
pub async fn members(
    State(state): State<AppState>,
    ctx: OrgContext,
) -> Result<Json<Vec<Member>>, ApiError> {
    Ok(Json(service::members(&state.db, &ctx).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/organization/members",
    tag = "Account",
    request_body = AddMember,
    responses(
        (status = 200, description = "Members after the change", body = Vec<Member>),
        (status = 403, description = "Requires ADMIN, and OWNER to grant OWNER", body = ErrorBody),
        (status = 422, description = "No account with that email yet", body = ErrorBody),
    )
)]
pub async fn add_member(
    State(state): State<AppState>,
    ctx: OrgContext,
    Json(body): Json<AddMember>,
) -> Result<Json<Vec<Member>>, ApiError> {
    Ok(Json(service::add_member(&state.db, &ctx, body).await?))
}

#[utoipa::path(
    patch,
    path = "/api/v1/organization/members/{user_id}",
    tag = "Account",
    params(("user_id" = Uuid, Path, description = "Member's user id")),
    request_body = UpdateMemberRole,
    responses(
        (status = 200, description = "Members after the change", body = Vec<Member>),
        (status = 403, description = "Requires ADMIN, and OWNER to grant OWNER", body = ErrorBody),
        (status = 409, description = "Would leave the organization with no owner", body = ErrorBody),
    )
)]
pub async fn set_role(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(user_id): Path<Uuid>,
    Json(body): Json<UpdateMemberRole>,
) -> Result<Json<Vec<Member>>, ApiError> {
    Ok(Json(service::set_role(&state.db, &ctx, user_id, body).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/organization/members/{user_id}",
    tag = "Account",
    params(("user_id" = Uuid, Path, description = "Member's user id")),
    responses(
        (status = 200, description = "Members after the removal", body = Vec<Member>),
        (status = 403, description = "Requires ADMIN", body = ErrorBody),
        (status = 409, description = "Would leave the organization with no owner", body = ErrorBody),
    )
)]
pub async fn remove_member(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(user_id): Path<Uuid>,
) -> Result<Json<Vec<Member>>, ApiError> {
    Ok(Json(service::remove_member(&state.db, &ctx, user_id).await?))
}
