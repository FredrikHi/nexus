use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;
use crate::util::slugify;

use super::dto::{AddTeamMember, CreateTeam, UpdateTeam};
use super::model::{Team, TeamMember};
use super::repository;

pub async fn list(db: &PgPool, org_id: Uuid) -> Result<Vec<Team>, ApiError> {
    repository::list(db, org_id).await
}

pub async fn get(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<Team, ApiError> {
    repository::find(db, org_id, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("team {id} not found")))
}

pub async fn create(db: &PgPool, org_id: Uuid, input: CreateTeam) -> Result<Team, ApiError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(ApiError::Validation("name must not be empty".to_string()));
    }

    let slug = match input.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => slugify(s),
        _ => slugify(name),
    };
    if slug.is_empty() {
        return Err(ApiError::Validation(
            "could not derive a slug; provide a slug explicitly".to_string(),
        ));
    }

    let id = repository::insert(db, org_id, name, &slug, input.description.as_deref()).await?;
    get(db, org_id, id).await
}

pub async fn update(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
    input: UpdateTeam,
) -> Result<Team, ApiError> {
    if let Some(name) = input.name.as_deref() {
        if name.trim().is_empty() {
            return Err(ApiError::Validation("name must not be empty".to_string()));
        }
    }

    if !repository::update(db, org_id, id, &input).await? {
        return Err(ApiError::NotFound(format!("team {id} not found")));
    }
    get(db, org_id, id).await
}

/// Deleting a team leaves what it owned in place, unowned.
///
/// The foreign keys are ON DELETE SET NULL, so no system or integration
/// disappears with the team that happened to be responsible for it.
pub async fn delete(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<(), ApiError> {
    if repository::delete(db, org_id, id).await? {
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("team {id} not found")))
    }
}

pub async fn members(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<Vec<TeamMember>, ApiError> {
    // Confirm the team is ours before reading its members, so this cannot be
    // used to enumerate another organization's people.
    get(db, org_id, id).await?;
    repository::members(db, id).await
}

pub async fn add_member(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
    input: AddTeamMember,
) -> Result<Vec<TeamMember>, ApiError> {
    get(db, org_id, id).await?;

    // A team may only contain people already in the organization. Without this
    // a team would become a way into a tenant you were never admitted to.
    if !repository::is_org_member(db, org_id, input.user_id).await? {
        return Err(ApiError::Validation(
            "that user is not a member of this organization".to_string(),
        ));
    }

    repository::add_member(db, id, input.user_id).await?;
    repository::members(db, id).await
}

pub async fn remove_member(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
    user_id: Uuid,
) -> Result<Vec<TeamMember>, ApiError> {
    get(db, org_id, id).await?;

    if !repository::remove_member(db, id, user_id).await? {
        return Err(ApiError::NotFound(
            "that user is not in this team".to_string(),
        ));
    }
    repository::members(db, id).await
}
