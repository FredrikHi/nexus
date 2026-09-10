use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::dto::{CreateIntegration, UpdateIntegration};
use super::model::{Integration, IntegrationRow, ReferenceCheck};

// Integrations carry their own organization_id, so unlike components no join
// is needed to scope a query to an org.

/// Lists integrations in an organization, optionally narrowed by environment
/// and/or by a component at either end of the edge.
///
/// Each filter is a `($n::uuid IS NULL OR ...)` clause, so a `None` bind simply
/// disables it. The statement stays static however many filters are combined:
/// no string concatenation, no injection surface.
pub async fn list(
    db: &PgPool,
    org_id: Uuid,
    environment_id: Option<Uuid>,
    component_id: Option<Uuid>,
) -> Result<Vec<Integration>, ApiError> {
    let rows = sqlx::query_as!(
        IntegrationRow,
        r#"SELECT id, organization_id, name, slug, description, source_component_id,
                  destination_component_id, integration_type_id, environment_id,
                  owner_team_id, criticality, status, documentation_url,
                  monitoring_enabled, health_check_enabled, metadata,
                  created_at, updated_at
           FROM integrations
           WHERE organization_id = $1
             AND ($2::uuid IS NULL OR environment_id = $2)
             AND ($3::uuid IS NULL OR source_component_id = $3
                                   OR destination_component_id = $3)
           ORDER BY name"#,
        org_id,
        environment_id,
        component_id
    )
    .fetch_all(db)
    .await?;

    rows.into_iter().map(IntegrationRow::into_domain).collect()
}

pub async fn find_by_id(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
) -> Result<Option<Integration>, ApiError> {
    let row = sqlx::query_as!(
        IntegrationRow,
        r#"SELECT id, organization_id, name, slug, description, source_component_id,
                  destination_component_id, integration_type_id, environment_id,
                  owner_team_id, criticality, status, documentation_url,
                  monitoring_enabled, health_check_enabled, metadata,
                  created_at, updated_at
           FROM integrations
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

/// Checks every referenced id in ONE round trip, returning a row of booleans.
///
/// The foreign keys would also catch a bad id, but only as a generic 422 that
/// names no field, and only for the first violation Postgres happens to hit.
/// Asking up front lets the service name every offending field at once.
///
/// environment_id is nullable, so its clause short-circuits to true when the
/// caller passed nothing to check. Every column here is a computed expression
/// rather than a table column, so each alias needs the `!` suffix to tell the
/// macro it is genuinely never NULL.
pub async fn check_references(
    db: &PgPool,
    org_id: Uuid,
    source_component_id: Uuid,
    destination_component_id: Uuid,
    integration_type_id: Uuid,
    environment_id: Option<Uuid>,
) -> Result<ReferenceCheck, ApiError> {
    let check = sqlx::query_as!(
        ReferenceCheck,
        r#"SELECT
             EXISTS (SELECT 1 FROM components c JOIN systems s ON s.id = c.system_id
                     WHERE c.id = $1 AND s.organization_id = $5) AS "source_exists!",
             EXISTS (SELECT 1 FROM components c JOIN systems s ON s.id = c.system_id
                     WHERE c.id = $2 AND s.organization_id = $5) AS "destination_exists!",
             EXISTS (SELECT 1 FROM integration_types
                     WHERE id = $3) AS "integration_type_exists!",
             ($4::uuid IS NULL
              OR EXISTS (SELECT 1 FROM environments
                         WHERE id = $4 AND organization_id = $5)) AS "environment_exists!""#,
        source_component_id,
        destination_component_id,
        integration_type_id,
        environment_id,
        org_id
    )
    .fetch_one(db)
    .await?;

    Ok(check)
}

pub async fn insert(
    db: &PgPool,
    org_id: Uuid,
    name: &str,
    slug: &str,
    input: &CreateIntegration,
    metadata: serde_json::Value,
) -> Result<Integration, ApiError> {
    let row = sqlx::query_as!(
        IntegrationRow,
        r#"INSERT INTO integrations
             (organization_id, name, slug, description, source_component_id,
              destination_component_id, integration_type_id, environment_id, owner_team_id,
              criticality, status, documentation_url, monitoring_enabled,
              health_check_enabled, metadata)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
           RETURNING id, organization_id, name, slug, description, source_component_id,
                     destination_component_id, integration_type_id, environment_id,
                     owner_team_id, criticality, status, documentation_url,
                     monitoring_enabled, health_check_enabled, metadata,
                     created_at, updated_at"#,
        org_id,
        name,
        slug,
        input.description.as_deref(),
        input.source_component_id,
        input.destination_component_id,
        input.integration_type_id,
        input.environment_id,
        input.owner_team_id,
        input.criticality.as_str(),
        input.status.as_str(),
        input.documentation_url.as_deref(),
        input.monitoring_enabled,
        input.health_check_enabled,
        metadata
    )
    .fetch_one(db)
    .await?;

    row.into_domain()
}

pub async fn update(
    db: &PgPool,
    id: Uuid,
    input: &UpdateIntegration,
) -> Result<Option<Integration>, ApiError> {
    // COALESCE($n, col): a NULL bind leaves the column untouched. One static
    // statement, no dynamic SQL. Trade-off: it cannot set a nullable column
    // back to NULL, which would need sqlx::QueryBuilder.
    let row = sqlx::query_as!(
        IntegrationRow,
        r#"UPDATE integrations SET
             name                     = COALESCE($2,  name),
             slug                     = COALESCE($3,  slug),
             description              = COALESCE($4,  description),
             source_component_id      = COALESCE($5,  source_component_id),
             destination_component_id = COALESCE($6,  destination_component_id),
             integration_type_id      = COALESCE($7,  integration_type_id),
             environment_id           = COALESCE($8,  environment_id),
             owner_team_id            = COALESCE($9,  owner_team_id),
             criticality              = COALESCE($10, criticality),
             status                   = COALESCE($11, status),
             documentation_url        = COALESCE($12, documentation_url),
             monitoring_enabled       = COALESCE($13, monitoring_enabled),
             health_check_enabled     = COALESCE($14, health_check_enabled),
             metadata                 = COALESCE($15, metadata)
           WHERE id = $1
           RETURNING id, organization_id, name, slug, description, source_component_id,
                     destination_component_id, integration_type_id, environment_id,
                     owner_team_id, criticality, status, documentation_url,
                     monitoring_enabled, health_check_enabled, metadata,
                     created_at, updated_at"#,
        id,
        input.name.as_deref(),
        input.slug.as_deref(),
        input.description.as_deref(),
        input.source_component_id,
        input.destination_component_id,
        input.integration_type_id,
        input.environment_id,
        input.owner_team_id,
        input.criticality.map(|c| c.as_str()),
        input.status.map(|s| s.as_str()),
        input.documentation_url.as_deref(),
        input.monitoring_enabled,
        input.health_check_enabled,
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
    let result = sqlx::query!("DELETE FROM integrations WHERE id = $1", id)
        .execute(db)
        .await?;

    Ok(result.rows_affected() > 0)
}
