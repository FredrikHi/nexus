use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;
use crate::util::{default_org_id, slugify};

use super::dto::{CreateSystem, UpdateSystem};
use super::model::System;
use super::repository;

pub async fn list(db: &PgPool) -> Result<Vec<System>, ApiError> {
    repository::list_by_org(db, default_org_id()).await
}

pub async fn get(db: &PgPool, id: Uuid) -> Result<System, ApiError> {
    repository::find_by_id(db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("system {id} not found")))
}

pub async fn create(db: &PgPool, input: CreateSystem) -> Result<System, ApiError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(ApiError::Validation("name must not be empty".to_string()));
    }

    // Use the provided slug, or derive one from the name.
    let slug = match input.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => slugify(s),
        _ => slugify(name),
    };
    if slug.is_empty() {
        return Err(ApiError::Validation(
            "could not derive a slug; provide a slug explicitly".to_string(),
        ));
    }

    // Coerce a missing metadata (JSON null) into an empty object.
    let metadata = if input.metadata.is_null() {
        serde_json::json!({})
    } else {
        input.metadata.clone()
    };

    repository::insert(db, default_org_id(), name, &slug, &input, metadata).await
}

pub async fn update(db: &PgPool, id: Uuid, input: UpdateSystem) -> Result<System, ApiError> {
    // 404 up front if it doesn't exist, so a PATCH to a missing id is clear.
    get(db, id).await?;

    if let Some(name) = input.name.as_deref() {
        if name.trim().is_empty() {
            return Err(ApiError::Validation("name must not be empty".to_string()));
        }
    }

    repository::update(db, id, &input)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("system {id} not found")))
}

pub async fn delete(db: &PgPool, id: Uuid) -> Result<(), ApiError> {
    if repository::delete(db, id).await? {
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("system {id} not found")))
    }
}
