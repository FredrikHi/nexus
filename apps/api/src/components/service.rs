use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;
use crate::util::{default_org_id, slugify};

use super::dto::{CreateComponent, UpdateComponent};
use super::model::Component;
use super::repository;

/// Lists components, optionally narrowed to a single system. Filtering by a
/// system that does not exist yields an empty list rather than a 404: a list
/// endpoint answers "what matches", and nothing matching is a valid answer.
pub async fn list(db: &PgPool, system_id: Option<Uuid>) -> Result<Vec<Component>, ApiError> {
    repository::list(db, default_org_id(), system_id).await
}

pub async fn get(db: &PgPool, id: Uuid) -> Result<Component, ApiError> {
    repository::find_by_id(db, default_org_id(), id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("component {id} not found")))
}

pub async fn create(db: &PgPool, input: CreateComponent) -> Result<Component, ApiError> {
    let org_id = default_org_id();

    let name = input.name.trim();
    if name.is_empty() {
        return Err(ApiError::Validation("name must not be empty".to_string()));
    }

    // Check the parent system up front. The foreign key would also catch this,
    // but only as a generic "a referenced resource does not exist" 422; naming
    // the offending field and id is far more useful to an API client.
    if !repository::system_exists(db, org_id, input.system_id).await? {
        return Err(ApiError::Validation(format!(
            "system_id {} does not exist",
            input.system_id
        )));
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

    // Coerce a missing metadata (JSON null) into an empty object, since the
    // column is NOT NULL.
    let metadata = if input.metadata.is_null() {
        serde_json::json!({})
    } else {
        input.metadata.clone()
    };

    repository::insert(db, name, &slug, &input, metadata).await
}

pub async fn update(db: &PgPool, id: Uuid, input: UpdateComponent) -> Result<Component, ApiError> {
    // 404 up front, so a PATCH against a missing id is unambiguous.
    get(db, id).await?;

    if let Some(name) = input.name.as_deref() {
        if name.trim().is_empty() {
            return Err(ApiError::Validation("name must not be empty".to_string()));
        }
    }

    if let Some(slug) = input.slug.as_deref() {
        if slugify(slug).is_empty() {
            return Err(ApiError::Validation(
                "slug must contain at least one alphanumeric character".to_string(),
            ));
        }
    }

    repository::update(db, id, &input)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("component {id} not found")))
}

pub async fn delete(db: &PgPool, id: Uuid) -> Result<(), ApiError> {
    // Scope the existence check to the organization before deleting, so a
    // component in another org reads as 404 rather than being removed.
    get(db, id).await?;

    if repository::delete(db, id).await? {
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("component {id} not found")))
    }
}
