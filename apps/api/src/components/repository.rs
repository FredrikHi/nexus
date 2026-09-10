use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::dto::{CreateComponent, UpdateComponent};
use super::model::{Component, ComponentRow};

// Components carry no organization_id of their own: they inherit it from the
// system they belong to, so every read joins `systems` to scope the query.

/// Lists components in an organization, optionally narrowed to one system.
///
/// The `($2::uuid IS NULL OR ...)` trick keeps this a single static statement
/// instead of concatenating SQL: a `None` bind disables the filter. The
/// explicit `::uuid` cast is required because Postgres cannot infer the type
/// of a bare parameter that only appears in an `IS NULL` test.
pub async fn list(
    db: &PgPool,
    org_id: Uuid,
    system_id: Option<Uuid>,
) -> Result<Vec<Component>, ApiError> {
    let rows = sqlx::query_as!(
        ComponentRow,
        r#"SELECT c.id, c.system_id, c.name, c.slug, c.description, c.component_type,
                  c.owner_team_id, c.repository_url, c.documentation_url, c.metadata,
                  c.created_at, c.updated_at
           FROM components c
           JOIN systems s ON s.id = c.system_id
           WHERE s.organization_id = $1
             AND ($2::uuid IS NULL OR c.system_id = $2)
           ORDER BY c.name"#,
        org_id,
        system_id
    )
    .fetch_all(db)
    .await?;

    rows.into_iter().map(ComponentRow::into_domain).collect()
}

pub async fn find_by_id(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<Component>, ApiError> {
    let row = sqlx::query_as!(
        ComponentRow,
        r#"SELECT c.id, c.system_id, c.name, c.slug, c.description, c.component_type,
                  c.owner_team_id, c.repository_url, c.documentation_url, c.metadata,
                  c.created_at, c.updated_at
           FROM components c
           JOIN systems s ON s.id = c.system_id
           WHERE c.id = $1 AND s.organization_id = $2"#,
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

/// Does this system exist inside this organization? Used by the service so a
/// bad `system_id` gets a precise message instead of the generic
/// "a referenced resource does not exist" that an FK violation would produce.
///
/// `EXISTS` is an expression, not a table column, so Postgres describes it as
/// nullable even though it never is. The `!` suffix on the alias is how you
/// tell the macro to trust you and produce a plain `bool`.
pub async fn system_exists(db: &PgPool, org_id: Uuid, system_id: Uuid) -> Result<bool, ApiError> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (
             SELECT 1 FROM systems WHERE id = $1 AND organization_id = $2
           ) AS "exists!""#,
        system_id,
        org_id
    )
    .fetch_one(db)
    .await?;

    Ok(exists)
}

pub async fn insert(
    db: &PgPool,
    name: &str,
    slug: &str,
    input: &CreateComponent,
    metadata: serde_json::Value,
) -> Result<Component, ApiError> {
    let row = sqlx::query_as!(
        ComponentRow,
        r#"INSERT INTO components
             (system_id, name, slug, description, component_type, owner_team_id,
              repository_url, documentation_url, metadata)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
           RETURNING id, system_id, name, slug, description, component_type,
                     owner_team_id, repository_url, documentation_url, metadata,
                     created_at, updated_at"#,
        input.system_id,
        name,
        slug,
        input.description.as_deref(),
        input.component_type.as_str(),
        input.owner_team_id,
        input.repository_url.as_deref(),
        input.documentation_url.as_deref(),
        metadata
    )
    .fetch_one(db)
    .await?;

    row.into_domain()
}

pub async fn update(
    db: &PgPool,
    id: Uuid,
    input: &UpdateComponent,
) -> Result<Option<Component>, ApiError> {
    // COALESCE($n, col): a NULL bind leaves the column untouched, a non-NULL
    // bind overwrites it. One static statement, no dynamic SQL. Trade-off: it
    // cannot set a nullable column back to NULL; that needs a query builder.
    let row = sqlx::query_as!(
        ComponentRow,
        r#"UPDATE components SET
             name              = COALESCE($2, name),
             slug              = COALESCE($3, slug),
             description       = COALESCE($4, description),
             component_type    = COALESCE($5, component_type),
             owner_team_id     = COALESCE($6, owner_team_id),
             repository_url    = COALESCE($7, repository_url),
             documentation_url = COALESCE($8, documentation_url),
             metadata          = COALESCE($9, metadata)
           WHERE id = $1
           RETURNING id, system_id, name, slug, description, component_type,
                     owner_team_id, repository_url, documentation_url, metadata,
                     created_at, updated_at"#,
        id,
        input.name.as_deref(),
        input.slug.as_deref(),
        input.description.as_deref(),
        input.component_type.map(|t| t.as_str()),
        input.owner_team_id,
        input.repository_url.as_deref(),
        input.documentation_url.as_deref(),
        input.metadata.clone()
    )
    .fetch_optional(db)
    .await?;

    match row {
        Some(r) => Ok(Some(r.into_domain()?)),
        None => Ok(None),
    }
}

pub async fn delete(db: &PgPool, id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query!("DELETE FROM components WHERE id = $1", id)
        .execute(db)
        .await?;

    Ok(result.rows_affected() > 0)
}
