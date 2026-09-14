use axum::extract::State;
use axum::http::StatusCode;
use uuid::Uuid;

use crate::auth::OrgContext;
use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path};
use crate::AppState;

use super::dto::{AddTeamMember, CreateTeam, UpdateTeam};
use super::model::{Team, TeamMember};
use super::service;

#[utoipa::path(
    get, path = "/api/v1/teams", tag = "Teams",
    responses((status = 200, description = "Teams, with what each owns", body = Vec<Team>))
)]
pub async fn list(
    State(state): State<AppState>,
    ctx: OrgContext,
) -> Result<Json<Vec<Team>>, ApiError> {
    Ok(Json(service::list(&state.db, ctx.organization_id).await?))
}

#[utoipa::path(
    post, path = "/api/v1/teams", tag = "Teams",
    request_body = CreateTeam,
    responses(
        (status = 201, description = "Created", body = Team),
        (status = 403, description = "Requires ADMIN", body = ErrorBody),
        (status = 409, description = "That slug is already used here", body = ErrorBody),
    )
)]
pub async fn create(
    State(state): State<AppState>,
    ctx: OrgContext,
    Json(body): Json<CreateTeam>,
) -> Result<(StatusCode, Json<Team>), ApiError> {
    // Teams decide who is responsible for what, which is an administrative
    // matter rather than ordinary editing.
    ctx.require_admin()?;
    let created = service::create(&state.db, ctx.organization_id, body).await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[utoipa::path(
    get, path = "/api/v1/teams/{id}", tag = "Teams",
    params(("id" = Uuid, Path, description = "Team id")),
    responses(
        (status = 200, description = "The team", body = Team),
        (status = 404, description = "No such team", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Team>, ApiError> {
    Ok(Json(
        service::get(&state.db, ctx.organization_id, id).await?,
    ))
}

#[utoipa::path(
    patch, path = "/api/v1/teams/{id}", tag = "Teams",
    params(("id" = Uuid, Path, description = "Team id")),
    request_body = UpdateTeam,
    responses(
        (status = 200, description = "Updated", body = Team),
        (status = 404, description = "No such team", body = ErrorBody),
    )
)]
pub async fn update(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateTeam>,
) -> Result<Json<Team>, ApiError> {
    ctx.require_admin()?;
    Ok(Json(
        service::update(&state.db, ctx.organization_id, id, body).await?,
    ))
}

#[utoipa::path(
    delete, path = "/api/v1/teams/{id}", tag = "Teams",
    params(("id" = Uuid, Path, description = "Team id")),
    responses(
        (status = 204,
         description = "Deleted. What it owned stays, unowned: nothing disappears with the team."),
        (status = 404, description = "No such team", body = ErrorBody),
    )
)]
pub async fn delete(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    ctx.require_admin()?;
    service::delete(&state.db, ctx.organization_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get, path = "/api/v1/teams/{id}/members", tag = "Teams",
    params(("id" = Uuid, Path, description = "Team id")),
    responses((status = 200, description = "People in the team", body = Vec<TeamMember>))
)]
pub async fn members(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<TeamMember>>, ApiError> {
    Ok(Json(
        service::members(&state.db, ctx.organization_id, id).await?,
    ))
}

#[utoipa::path(
    post, path = "/api/v1/teams/{id}/members", tag = "Teams",
    params(("id" = Uuid, Path, description = "Team id")),
    request_body = AddTeamMember,
    responses(
        (status = 200, description = "Members after the change", body = Vec<TeamMember>),
        (status = 422, description = "That user is not in this organization", body = ErrorBody),
    )
)]
pub async fn add_member(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path(id): Path<Uuid>,
    Json(body): Json<AddTeamMember>,
) -> Result<Json<Vec<TeamMember>>, ApiError> {
    ctx.require_admin()?;
    Ok(Json(
        service::add_member(&state.db, ctx.organization_id, id, body).await?,
    ))
}

#[utoipa::path(
    delete, path = "/api/v1/teams/{id}/members/{user_id}", tag = "Teams",
    params(
        ("id" = Uuid, Path, description = "Team id"),
        ("user_id" = Uuid, Path, description = "User id"),
    ),
    responses(
        (status = 200, description = "Members after the removal", body = Vec<TeamMember>),
        (status = 404, description = "That user is not in this team", body = ErrorBody),
    )
)]
pub async fn remove_member(
    State(state): State<AppState>,
    ctx: OrgContext,
    Path((id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<TeamMember>>, ApiError> {
    ctx.require_admin()?;
    Ok(Json(
        service::remove_member(&state.db, ctx.organization_id, id, user_id).await?,
    ))
}
