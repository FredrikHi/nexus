use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;
use crate::util::slugify;

use super::dto::{CreateIntegration, UpdateIntegration};
use super::model::Integration;
use super::repository;

pub async fn list(
    db: &PgPool,
    org_id: Uuid,
    environment_id: Option<Uuid>,
    component_id: Option<Uuid>,
) -> Result<Vec<Integration>, ApiError> {
    repository::list(db, org_id, environment_id, component_id).await
}

pub async fn get(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<Integration, ApiError> {
    repository::find_by_id(db, org_id, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("integration {id} not found")))
}

/// Validates the four referenced ids and the self-loop rule together, so the
/// caller gets every problem in one response instead of fixing them one per
/// request. Returns `Ok(())` when the whole set is sound.
async fn validate_references(
    db: &PgPool,
    org_id: Uuid,
    source_component_id: Uuid,
    destination_component_id: Uuid,
    integration_type_id: Uuid,
    environment_id: Option<Uuid>,
) -> Result<(), ApiError> {
    // Collect problems rather than returning on the first one. `Vec<String>`
    // here is doing the job a validation-errors list does in a C# model binder.
    let mut problems: Vec<String> = Vec::new();

    // An integration is an edge between two components. An edge from a node to
    // itself carries no information and would corrupt any topology built from
    // these rows later, so it is rejected outright.
    if source_component_id == destination_component_id {
        problems.push("source_component_id and destination_component_id must differ".to_string());
    }

    let check = repository::check_references(
        db,
        org_id,
        source_component_id,
        destination_component_id,
        integration_type_id,
        environment_id,
    )
    .await?;

    if !check.source_exists {
        problems.push(format!(
            "source_component_id {source_component_id} does not exist"
        ));
    }
    if !check.destination_exists {
        problems.push(format!(
            "destination_component_id {destination_component_id} does not exist"
        ));
    }
    if !check.integration_type_exists {
        problems.push(format!(
            "integration_type_id {integration_type_id} does not exist"
        ));
    }
    if !check.environment_exists {
        // Only reachable when the caller supplied an id, since the SQL clause
        // short-circuits to true for NULL.
        if let Some(env_id) = environment_id {
            problems.push(format!("environment_id {env_id} does not exist"));
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(ApiError::Validation(problems.join("; ")))
    }
}

pub async fn create(
    db: &PgPool,
    org_id: Uuid,
    input: CreateIntegration,
) -> Result<Integration, ApiError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(ApiError::Validation("name must not be empty".to_string()));
    }

    validate_references(
        db,
        org_id,
        input.source_component_id,
        input.destination_component_id,
        input.integration_type_id,
        input.environment_id,
    )
    .await?;

    let slug = match input.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => slugify(s),
        _ => slugify(name),
    };
    if slug.is_empty() {
        return Err(ApiError::Validation(
            "could not derive a slug; provide a slug explicitly".to_string(),
        ));
    }

    // The column is NOT NULL, so a missing metadata (JSON null) becomes {}.
    let metadata = if input.metadata.is_null() {
        serde_json::json!({})
    } else {
        input.metadata.clone()
    };

    repository::insert(db, org_id, name, &slug, &input, metadata).await
}

pub async fn update(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
    input: UpdateIntegration,
) -> Result<Integration, ApiError> {
    // 404 up front, and the current row doubles as the base for the checks
    // below: a PATCH that changes only one endpoint still has to be validated
    // against the endpoint it is keeping.
    let current = get(db, org_id, id).await?;

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

    // Re-validate only when the patch actually touches a reference. `or` on an
    // Option reads as "the new value if given, otherwise the existing one".
    let touches_references = input.source_component_id.is_some()
        || input.destination_component_id.is_some()
        || input.integration_type_id.is_some()
        || input.environment_id.is_some();

    if touches_references {
        validate_references(
            db,
            org_id,
            input
                .source_component_id
                .unwrap_or(current.source_component_id),
            input
                .destination_component_id
                .unwrap_or(current.destination_component_id),
            input
                .integration_type_id
                .unwrap_or(current.integration_type_id),
            input.environment_id.or(current.environment_id),
        )
        .await?;
    }

    repository::update(db, id, &input)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("integration {id} not found")))
}

pub async fn delete(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<(), ApiError> {
    // Scope the existence check to the organization first, so an integration
    // in another org reads as 404 rather than being removed.
    get(db, org_id, id).await?;

    if repository::delete(db, id).await? {
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("integration {id} not found")))
    }
}
