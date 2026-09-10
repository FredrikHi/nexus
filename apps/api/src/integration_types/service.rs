use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;
use crate::util::slugify;

use super::dto::CreateIntegrationType;
use super::model::IntegrationType;
use super::repository;

pub async fn list(db: &PgPool, org_id: Uuid) -> Result<Vec<IntegrationType>, ApiError> {
    repository::list(db, org_id).await
}

pub async fn create(
    db: &PgPool,
    org_id: Uuid,
    input: CreateIntegrationType,
) -> Result<IntegrationType, ApiError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(ApiError::Validation("name must not be empty".to_string()));
    }

    // Keys follow the SCREAMING_SNAKE convention the built-ins use, so a
    // custom type reads the same as HTTP or MESSAGE_QUEUE in the UI.
    let key = match input.key.as_deref() {
        Some(k) if !k.trim().is_empty() => slugify(k),
        _ => slugify(name),
    };
    if key.is_empty() {
        return Err(ApiError::Validation(
            "could not derive a key; provide one explicitly".to_string(),
        ));
    }
    let key = key.to_uppercase().replace('-', "_");

    repository::insert(db, org_id, &key, name, input.description.as_deref()).await
}

/// Removes a custom type. Built-ins are not deletable by anyone, and a type
/// still in use is refused rather than left to fail as a constraint error.
pub async fn delete(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<(), ApiError> {
    let in_use = repository::integrations_using(db, id).await?;
    if in_use > 0 {
        return Err(ApiError::Conflict(format!(
            "{in_use} integration(s) still use this type; change them first"
        )));
    }

    if repository::delete(db, org_id, id).await? {
        Ok(())
    } else {
        Err(ApiError::NotFound(
            "no such custom integration type in this organization; built-ins cannot be deleted"
                .to_string(),
        ))
    }
}
