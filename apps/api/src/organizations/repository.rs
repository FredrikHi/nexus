use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::model::{MemberRow, Organization};

/// Creates an organization and makes the creator its owner, atomically.
///
/// A transaction because the two writes are one act: an organization with no
/// owner would be unreachable by anyone, including the person who just made
/// it. Either both land or neither does.
pub async fn create_with_owner(
    db: &PgPool,
    name: &str,
    slug: &str,
    description: Option<&str>,
    owner_id: Uuid,
) -> Result<Organization, ApiError> {
    let mut tx = db.begin().await?;

    let organization = sqlx::query_as!(
        Organization,
        r#"INSERT INTO organizations (name, slug, description)
           VALUES ($1,$2,$3)
           RETURNING id, name, slug AS "slug!", description, created_at"#,
        name,
        slug,
        description
    )
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query!(
        "INSERT INTO organization_members (organization_id, user_id, role) VALUES ($1,$2,'OWNER')",
        organization.id,
        owner_id
    )
    .execute(&mut *tx)
    .await?;

    // Every default policy the platform needs for a brand new tenant. Without
    // this the health worker would evaluate the organization against nothing.
    sqlx::query!(
        "INSERT INTO health_policies (organization_id, integration_id) VALUES ($1, NULL)",
        organization.id
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(organization)
}

pub async fn find(db: &PgPool, id: Uuid) -> Result<Option<Organization>, ApiError> {
    let organization = sqlx::query_as!(
        Organization,
        r#"SELECT id, name, slug AS "slug!", description, created_at
           FROM organizations WHERE id = $1"#,
        id
    )
    .fetch_optional(db)
    .await?;

    Ok(organization)
}

pub async fn members(db: &PgPool, organization_id: Uuid) -> Result<Vec<MemberRow>, ApiError> {
    let rows = sqlx::query_as!(
        MemberRow,
        r#"SELECT u.id AS "user_id!", u.email AS "email!", u.display_name AS "display_name!",
                  u.avatar_url, m.role AS "role!", m.created_at AS "joined_at!"
           FROM organization_members m
           JOIN users u ON u.id = m.user_id
           WHERE m.organization_id = $1
           ORDER BY m.role DESC, u.display_name"#,
        organization_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn find_user_by_email(db: &PgPool, email: &str) -> Result<Option<Uuid>, ApiError> {
    let id = sqlx::query_scalar!(
        r#"SELECT id AS "id!" FROM users WHERE lower(email) = lower($1)"#,
        email
    )
    .fetch_optional(db)
    .await?;

    Ok(id)
}

pub async fn add_member(
    db: &PgPool,
    organization_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<(), ApiError> {
    sqlx::query!(
        r#"INSERT INTO organization_members (organization_id, user_id, role)
           VALUES ($1,$2,$3)
           ON CONFLICT (organization_id, user_id) DO UPDATE SET role = EXCLUDED.role"#,
        organization_id,
        user_id,
        role
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn set_role(
    db: &PgPool,
    organization_id: Uuid,
    user_id: Uuid,
    role: &str,
) -> Result<bool, ApiError> {
    let result = sqlx::query!(
        "UPDATE organization_members SET role = $3 WHERE organization_id = $1 AND user_id = $2",
        organization_id,
        user_id,
        role
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn remove_member(
    db: &PgPool,
    organization_id: Uuid,
    user_id: Uuid,
) -> Result<bool, ApiError> {
    let result = sqlx::query!(
        "DELETE FROM organization_members WHERE organization_id = $1 AND user_id = $2",
        organization_id,
        user_id
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// How many owners an organization has.
///
/// Guards the rule that an organization can never be left without one: losing
/// the last owner would make it permanently unmanageable, since only an owner
/// can appoint another.
pub async fn owner_count(db: &PgPool, organization_id: Uuid) -> Result<i64, ApiError> {
    let count = sqlx::query_scalar!(
        r#"SELECT count(*) AS "n!" FROM organization_members
           WHERE organization_id = $1 AND role = 'OWNER'"#,
        organization_id
    )
    .fetch_one(db)
    .await?;

    Ok(count)
}

/// Every organization on the platform.
///
/// Used by the background worker, which must evaluate all tenants rather than
/// one: a hosted deployment has many, and a self-hosted one may still have
/// several. This is the query that replaced the single-tenant assumption.
pub async fn all_ids(db: &PgPool) -> Result<Vec<Uuid>, ApiError> {
    let ids = sqlx::query_scalar!(r#"SELECT id AS "id!" FROM organizations ORDER BY created_at"#)
        .fetch_all(db)
        .await?;

    Ok(ids)
}
