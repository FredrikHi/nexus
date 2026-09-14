use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::dto::UpdateTeam;
use super::model::{Team, TeamMember};

/// Teams with their counts, in one query.
///
/// The counts are correlated subqueries rather than joins: joining three
/// one-to-many relations at once would multiply rows together and need a
/// DISTINCT to undo, which is both slower and easier to get wrong.
pub async fn list(db: &PgPool, org_id: Uuid) -> Result<Vec<Team>, ApiError> {
    let rows = sqlx::query_as!(
        Team,
        r#"SELECT
             t.id, t.organization_id, t.name, t.slug, t.description,
             (SELECT count(*) FROM team_members m WHERE m.team_id = t.id)      AS "member_count!",
             (SELECT count(*) FROM systems s WHERE s.owner_team_id = t.id)     AS "owned_systems!",
             (SELECT count(*) FROM integrations i WHERE i.owner_team_id = t.id) AS "owned_integrations!",
             t.created_at, t.updated_at
           FROM teams t
           WHERE t.organization_id = $1
           ORDER BY t.name"#,
        org_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn find(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<Option<Team>, ApiError> {
    let row = sqlx::query_as!(
        Team,
        r#"SELECT
             t.id, t.organization_id, t.name, t.slug, t.description,
             (SELECT count(*) FROM team_members m WHERE m.team_id = t.id)      AS "member_count!",
             (SELECT count(*) FROM systems s WHERE s.owner_team_id = t.id)     AS "owned_systems!",
             (SELECT count(*) FROM integrations i WHERE i.owner_team_id = t.id) AS "owned_integrations!",
             t.created_at, t.updated_at
           FROM teams t
           WHERE t.organization_id = $1 AND t.id = $2"#,
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
) -> Result<Uuid, ApiError> {
    let id = sqlx::query_scalar!(
        r#"INSERT INTO teams (organization_id, name, slug, description)
           VALUES ($1,$2,$3,$4) RETURNING id"#,
        org_id,
        name,
        slug,
        description
    )
    .fetch_one(db)
    .await?;

    Ok(id)
}

pub async fn update(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
    input: &UpdateTeam,
) -> Result<bool, ApiError> {
    let result = sqlx::query!(
        r#"UPDATE teams SET
             name        = COALESCE($3, name),
             slug        = COALESCE($4, slug),
             description = COALESCE($5, description)
           WHERE organization_id = $1 AND id = $2"#,
        org_id,
        id,
        input.name.as_deref(),
        input.slug.as_deref(),
        input.description.as_deref()
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn delete(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query!(
        "DELETE FROM teams WHERE organization_id = $1 AND id = $2",
        org_id,
        id
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn members(db: &PgPool, team_id: Uuid) -> Result<Vec<TeamMember>, ApiError> {
    let rows = sqlx::query_as!(
        TeamMember,
        r#"SELECT u.id AS "user_id!", u.email AS "email!",
                  u.display_name AS "display_name!", u.avatar_url
           FROM team_members m
           JOIN users u ON u.id = m.user_id
           WHERE m.team_id = $1
           ORDER BY u.display_name"#,
        team_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// Is this user a member of this organization?
///
/// A team may only contain people who are already in the organization;
/// otherwise a team would become a back door into a tenant.
pub async fn is_org_member(db: &PgPool, org_id: Uuid, user_id: Uuid) -> Result<bool, ApiError> {
    let exists = sqlx::query_scalar!(
        r#"SELECT EXISTS (
             SELECT 1 FROM organization_members
             WHERE organization_id = $1 AND user_id = $2
           ) AS "exists!""#,
        org_id,
        user_id
    )
    .fetch_one(db)
    .await?;

    Ok(exists)
}

pub async fn add_member(db: &PgPool, team_id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
    sqlx::query!(
        r#"INSERT INTO team_members (team_id, user_id) VALUES ($1,$2)
           ON CONFLICT (team_id, user_id) DO NOTHING"#,
        team_id,
        user_id
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn remove_member(db: &PgPool, team_id: Uuid, user_id: Uuid) -> Result<bool, ApiError> {
    let result = sqlx::query!(
        "DELETE FROM team_members WHERE team_id = $1 AND user_id = $2",
        team_id,
        user_id
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}
