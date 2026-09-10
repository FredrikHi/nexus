use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::OrgContext;
use crate::error::ApiError;
use crate::util::slugify;

use super::dto::{CreateEnvironment, UpdateEnvironment};
use super::model::Environment;
use super::repository;

pub async fn list(db: &PgPool, org_id: Uuid) -> Result<Vec<Environment>, ApiError> {
    repository::list(db, org_id).await
}

pub async fn get(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<Environment, ApiError> {
    repository::find(db, org_id, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("environment {id} not found")))
}

pub async fn create(
    db: &PgPool,
    org_id: Uuid,
    input: CreateEnvironment,
) -> Result<Environment, ApiError> {
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

    repository::insert(
        db,
        org_id,
        name,
        &slug,
        input.description.as_deref(),
        input.is_production,
    )
    .await
}

pub async fn update(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
    input: UpdateEnvironment,
) -> Result<Environment, ApiError> {
    if let Some(name) = input.name.as_deref() {
        if name.trim().is_empty() {
            return Err(ApiError::Validation("name must not be empty".to_string()));
        }
    }

    repository::update(db, org_id, id, &input)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("environment {id} not found")))
}

/// Deleting an environment nulls it out on every integration that used it.
///
/// The foreign key would do that silently, so this refuses up front unless the
/// caller says they mean it. Losing which environment an integration belongs
/// to is quiet and hard to notice afterwards.
pub async fn delete(
    db: &PgPool,
    ctx: &OrgContext,
    id: Uuid,
    force: bool,
) -> Result<(), ApiError> {
    get(db, ctx.organization_id, id).await?;

    let in_use = repository::integrations_using(db, id).await?;
    if in_use > 0 && !force {
        return Err(ApiError::Conflict(format!(
            "{in_use} integration(s) still use this environment; delete with force=true to detach them"
        )));
    }

    if repository::delete(db, ctx.organization_id, id).await? {
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("environment {id} not found")))
    }
}
