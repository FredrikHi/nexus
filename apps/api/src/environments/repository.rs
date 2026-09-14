use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::dto::UpdateEnvironment;
use super::model::Environment;

pub async fn list(db: &PgPool, org_id: Uuid) -> Result<Vec<Environment>, ApiError> {
    let rows = sqlx::query_as!(
        Environment,
        r#"SELECT id, organization_id, name, slug, description,
                  is_production, created_at, updated_at
           FROM environments
           WHERE organization_id = $1
           ORDER BY is_production, name"#,
        org_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn find(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<Option<Environment>, ApiError> {
    let row = sqlx::query_as!(
        Environment,
        r#"SELECT id, organization_id, name, slug, description,
                  is_production, created_at, updated_at
           FROM environments
           WHERE organization_id = $1 AND id = $2"#,
        org_id,
        id
    )
    .fetch_optional(db)
    .await?;

    Ok(row)
}

pub async fn insert(
    db: &PgPool,
    org_id: Uuid,
    name: &str,
    slug: &str,
    description: Option<&str>,
    is_production: bool,
) -> Result<Environment, ApiError> {
    let row = sqlx::query_as!(
        Environment,
        r#"INSERT INTO environments (organization_id, name, slug, description, is_production)
           VALUES ($1,$2,$3,$4,$5)
           RETURNING id, organization_id, name, slug, description,
                     is_production, created_at, updated_at"#,
        org_id,
        name,
        slug,
        description,
        is_production
    )
    .fetch_one(db)
    .await?;

    Ok(row)
}

pub async fn update(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
    input: &UpdateEnvironment,
) -> Result<Option<Environment>, ApiError> {
    let row = sqlx::query_as!(
        Environment,
        r#"UPDATE environments SET
             name          = COALESCE($3, name),
             slug          = COALESCE($4, slug),
             description   = COALESCE($5, description),
             is_production = COALESCE($6, is_production)
           WHERE organization_id = $1 AND id = $2
           RETURNING id, organization_id, name, slug, description,
                     is_production, created_at, updated_at"#,
        org_id,
        id,
        input.name.as_deref(),
        input.slug.as_deref(),
        input.description.as_deref(),
        input.is_production
    )
    .fetch_optional(db)
    .await?;

    Ok(row)
}

pub async fn delete(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query!(
        "DELETE FROM environments WHERE organization_id = $1 AND id = $2",
        org_id,
        id
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// How many integrations still point at this environment.
///
/// The foreign key would null them out silently on delete; telling the caller
/// first turns a surprise into a decision.
pub async fn integrations_using(db: &PgPool, environment_id: Uuid) -> Result<i64, ApiError> {
    let count = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM integrations WHERE environment_id = $1"#,
        environment_id
    )
    .fetch_one(db)
    .await?;

    Ok(count)
}
