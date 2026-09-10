use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::model::IntegrationType;

/// The built-ins plus this organization's own, built-ins first.
pub async fn list(db: &PgPool, org_id: Uuid) -> Result<Vec<IntegrationType>, ApiError> {
    let rows = sqlx::query_as!(
        IntegrationType,
        r#"SELECT id, key, name, description, is_builtin, organization_id, created_at
           FROM integration_types
           WHERE organization_id IS NULL OR organization_id = $1
           ORDER BY is_builtin DESC, name"#,
        org_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn insert(
    db: &PgPool,
    org_id: Uuid,
    key: &str,
    name: &str,
    description: Option<&str>,
) -> Result<IntegrationType, ApiError> {
    let row = sqlx::query_as!(
        IntegrationType,
        r#"INSERT INTO integration_types (organization_id, key, name, description, is_builtin)
           VALUES ($1,$2,$3,$4,false)
           RETURNING id, key, name, description, is_builtin, organization_id, created_at"#,
        org_id,
        key,
        name,
        description
    )
    .fetch_one(db)
    .await?;

    Ok(row)
}

/// Deletes a custom type belonging to this organization.
///
/// Scoped to `organization_id = $2` rather than checking `is_builtin`, so a
/// built-in simply does not match and cannot be removed by anyone.
pub async fn delete(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query!(
        "DELETE FROM integration_types WHERE id = $1 AND organization_id = $2",
        id,
        org_id
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// How many integrations are declared as this type.
///
/// The foreign key has no ON DELETE clause, so the database would refuse the
/// delete with a constraint error. Counting first turns that into a sentence.
pub async fn integrations_using(db: &PgPool, type_id: Uuid) -> Result<i64, ApiError> {
    let count = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM integrations WHERE integration_type_id = $1"#,
        type_id
    )
    .fetch_one(db)
    .await?;

    Ok(count)
}
