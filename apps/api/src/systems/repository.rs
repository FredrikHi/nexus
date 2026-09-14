use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::dto::{CreateSystem, UpdateSystem};
use super::model::{System, SystemRow};

// `query_as!` is a procedural macro: at COMPILE time it connects to Postgres
// (or reads the committed .sqlx cache), asks the server to describe the exact
// statement, and checks the result columns against the target struct. That is
// why the SQL must be a string literal rather than something built with
// `format!` -- a macro cannot run your code to find out what the query says.
//
// The cost of that is the column lists below, spelled out per statement. The
// shared `columns()` helper this file used to have could not survive.

pub async fn list_by_org(db: &PgPool, org_id: Uuid) -> Result<Vec<System>, ApiError> {
    let rows = sqlx::query_as!(
        SystemRow,
        r#"SELECT id, organization_id, name, slug, description, system_type,
                  owner_team_id, documentation_url, repository_url, criticality,
                  lifecycle_status, metadata, created_at, updated_at
           FROM systems
           WHERE organization_id = $1
           ORDER BY name"#,
        org_id
    )
    .fetch_all(db)
    .await?;

    // Convert every row into the domain type; the first bad row short-circuits.
    rows.into_iter().map(SystemRow::into_domain).collect()
}

/// Scoped to an organization. Without that filter a caller could read another
/// tenant's system simply by knowing its id.
pub async fn find_by_id(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<Option<System>, ApiError> {
    let row = sqlx::query_as!(
        SystemRow,
        r#"SELECT id, organization_id, name, slug, description, system_type,
                  owner_team_id, documentation_url, repository_url, criticality,
                  lifecycle_status, metadata, created_at, updated_at
           FROM systems
           WHERE id = $1 AND organization_id = $2"#,
        id,
        org_id
    )
    .fetch_optional(db)
    .await?;

    match row {
        Some(r) => Ok(Some(r.into_domain()?)),
        None => Ok(None),
    }
}

pub async fn insert(
    db: &PgPool,
    org_id: Uuid,
    name: &str,
    slug: &str,
    input: &CreateSystem,
    metadata: serde_json::Value,
) -> Result<System, ApiError> {
    let row = sqlx::query_as!(
        SystemRow,
        r#"INSERT INTO systems
             (organization_id, name, slug, description, system_type, owner_team_id,
              documentation_url, repository_url, criticality, lifecycle_status, metadata)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
           RETURNING id, organization_id, name, slug, description, system_type,
                     owner_team_id, documentation_url, repository_url, criticality,
                     lifecycle_status, metadata, created_at, updated_at"#,
        org_id,
        name,
        slug,
        input.description.as_deref(),
        input.system_type.as_str(),
        input.owner_team_id,
        input.documentation_url.as_deref(),
        input.repository_url.as_deref(),
        input.criticality.as_str(),
        input.lifecycle_status.as_str(),
        metadata
    )
    .fetch_one(db)
    .await?;

    row.into_domain()
}

pub async fn update(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
    input: &UpdateSystem,
) -> Result<Option<System>, ApiError> {
    // COALESCE($n, col): a NULL bind leaves the column untouched, a non-NULL
    // bind overwrites it. Simple and one static statement. Trade-off: it can't
    // set a nullable column back to NULL (that needs a dynamic query builder).
    let row = sqlx::query_as!(
        SystemRow,
        r#"UPDATE systems SET
             name              = COALESCE($2, name),
             slug              = COALESCE($3, slug),
             description       = COALESCE($4, description),
             system_type       = COALESCE($5, system_type),
             owner_team_id     = COALESCE($6, owner_team_id),
             documentation_url = COALESCE($7, documentation_url),
             repository_url    = COALESCE($8, repository_url),
             criticality       = COALESCE($9, criticality),
             lifecycle_status  = COALESCE($10, lifecycle_status),
             metadata          = COALESCE($11, metadata)
           WHERE id = $1 AND organization_id = $12
           RETURNING id, organization_id, name, slug, description, system_type,
                     owner_team_id, documentation_url, repository_url, criticality,
                     lifecycle_status, metadata, created_at, updated_at"#,
        id,
        input.name.as_deref(),
        input.slug.as_deref(),
        input.description.as_deref(),
        input.system_type.map(|t| t.as_str()),
        input.owner_team_id,
        input.documentation_url.as_deref(),
        input.repository_url.as_deref(),
        input.criticality.map(|c| c.as_str()),
        input.lifecycle_status.map(|l| l.as_str()),
        input.metadata.clone(),
        org_id
    )
    .fetch_optional(db)
    .await?;

    match row {
        Some(r) => Ok(Some(r.into_domain()?)),
        None => Ok(None),
    }
}

pub async fn delete(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query!(
        "DELETE FROM systems WHERE id = $1 AND organization_id = $2",
        id,
        org_id
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}
