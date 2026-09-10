use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::model::{Identity, MembershipRow};

/// Finds the local user for a token subject, creating one on first sight.
///
/// The auth service owns accounts; this row exists so the domain can have
/// foreign keys to a user without reaching into that service's tables. The
/// upsert refreshes the profile on every sign-in, so a changed display name or
/// avatar follows along without a sync job.
pub async fn upsert_by_subject(
    db: &PgPool,
    subject: &str,
    email: &str,
    display_name: &str,
    avatar_url: Option<&str>,
) -> Result<Identity, ApiError> {
    let identity = sqlx::query_as!(
        Identity,
        r#"INSERT INTO users (subject, email, display_name, avatar_url)
           VALUES ($1,$2,$3,$4)
           ON CONFLICT (subject) DO UPDATE SET
             email        = EXCLUDED.email,
             display_name = EXCLUDED.display_name,
             avatar_url   = EXCLUDED.avatar_url
           RETURNING id AS "user_id!", subject AS "subject!", email AS "email!",
                     display_name AS "display_name!", avatar_url"#,
        subject,
        email,
        display_name,
        avatar_url
    )
    .fetch_one(db)
    .await?;

    Ok(identity)
}

/// The caller's role in one organization, or None if they are not a member.
///
/// This is the check that makes the active-organization header safe: the token
/// proves who you are, and this proves you are allowed to act there.
pub async fn membership(
    db: &PgPool,
    user_id: Uuid,
    organization_id: Uuid,
) -> Result<Option<String>, ApiError> {
    let role = sqlx::query_scalar!(
        r#"SELECT role AS "role!"
           FROM organization_members
           WHERE user_id = $1 AND organization_id = $2"#,
        user_id,
        organization_id
    )
    .fetch_optional(db)
    .await?;

    Ok(role)
}

/// Every organization the caller belongs to, for the switcher.
pub async fn memberships(db: &PgPool, user_id: Uuid) -> Result<Vec<MembershipRow>, ApiError> {
    let rows = sqlx::query_as!(
        MembershipRow,
        r#"SELECT o.id AS "organization_id!", o.name AS "organization_name!",
                  o.slug AS "organization_slug!", m.role AS "role!", m.created_at AS "joined_at!"
           FROM organization_members m
           JOIN organizations o ON o.id = m.organization_id
           WHERE m.user_id = $1
           ORDER BY o.name"#,
        user_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// Up to two of the caller's organization ids.
///
/// Two is all the extractor needs: none means they should create one, exactly
/// one can be assumed, and more than one has to be stated explicitly. Guessing
/// between two would silently write into the wrong tenant.
pub async fn first_two_memberships(db: &PgPool, user_id: Uuid) -> Result<Vec<Uuid>, ApiError> {
    let rows = sqlx::query_scalar!(
        r#"SELECT organization_id AS "organization_id!"
           FROM organization_members
           WHERE user_id = $1
           LIMIT 2"#,
        user_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}
