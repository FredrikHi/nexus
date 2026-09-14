use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::{self, Identity, OrgContext, OrgRole};
use crate::error::ApiError;
use crate::util::slugify;

use super::dto::{AddMember, CreateOrganization, UpdateMemberRole};
use super::model::{Me, Member, Organization};
use super::repository;

/// Who the caller is and where they belong. The first call a client makes
/// after signing in, and what populates the organization switcher.
pub async fn me(db: &PgPool, identity: Identity) -> Result<Me, ApiError> {
    let organizations = auth::memberships(db, identity.user_id)
        .await?
        .into_iter()
        .map(|r| r.into_domain())
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Me {
        user_id: identity.user_id,
        email: identity.email,
        display_name: identity.display_name,
        avatar_url: identity.avatar_url,
        organizations,
    })
}

/// Anyone signed in may create an organization, and becomes its owner.
///
/// Deliberately not restricted: this is how a new account gets its first
/// tenant, and there is nobody to ask for permission yet.
pub async fn create(
    db: &PgPool,
    identity: &Identity,
    input: CreateOrganization,
) -> Result<Organization, ApiError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(ApiError::Validation("name must not be empty".to_string()));
    }

    let slug = match input.slug.as_deref() {
        Some(s) if !s.trim().is_empty() => slugify(s),
        _ => slugify(name),
    };
    if slug.is_empty() {
        return Err(ApiError::Validation(
            "could not derive a slug; provide a slug explicitly".to_string(),
        ));
    }

    repository::create_with_owner(
        db,
        name,
        &slug,
        input.description.as_deref(),
        identity.user_id,
    )
    .await
}

pub async fn get(db: &PgPool, ctx: &OrgContext) -> Result<Organization, ApiError> {
    repository::find(db, ctx.organization_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("organization not found".to_string()))
}

pub async fn members(db: &PgPool, ctx: &OrgContext) -> Result<Vec<Member>, ApiError> {
    repository::members(db, ctx.organization_id)
        .await?
        .into_iter()
        .map(|r| r.into_domain())
        .collect()
}

pub async fn add_member(
    db: &PgPool,
    ctx: &OrgContext,
    input: AddMember,
) -> Result<Vec<Member>, ApiError> {
    ctx.require_admin()?;

    // Only an owner may create another owner: otherwise an admin could
    // promote themselves past the person who invited them.
    if input.role == OrgRole::Owner && ctx.role != OrgRole::Owner {
        return Err(ApiError::Forbidden(
            "only an owner may grant the OWNER role".to_string(),
        ));
    }

    let email = input.email.trim();
    if email.is_empty() {
        return Err(ApiError::Validation("email must not be empty".to_string()));
    }

    // There is no invitation flow yet, so the account has to exist. Saying so
    // plainly beats a silent no-op or a confusing foreign key error.
    let user_id = repository::find_user_by_email(db, email)
        .await?
        .ok_or_else(|| {
            ApiError::Validation(format!(
                "no account for {email}; they must sign in once before being added"
            ))
        })?;

    repository::add_member(db, ctx.organization_id, user_id, input.role.as_str()).await?;
    members(db, ctx).await
}

pub async fn set_role(
    db: &PgPool,
    ctx: &OrgContext,
    user_id: Uuid,
    input: UpdateMemberRole,
) -> Result<Vec<Member>, ApiError> {
    ctx.require_admin()?;

    if input.role == OrgRole::Owner && ctx.role != OrgRole::Owner {
        return Err(ApiError::Forbidden(
            "only an owner may grant the OWNER role".to_string(),
        ));
    }

    guard_last_owner(db, ctx, user_id, input.role != OrgRole::Owner).await?;

    if !repository::set_role(db, ctx.organization_id, user_id, input.role.as_str()).await? {
        return Err(ApiError::NotFound("that user is not a member".to_string()));
    }

    members(db, ctx).await
}

pub async fn remove_member(
    db: &PgPool,
    ctx: &OrgContext,
    user_id: Uuid,
) -> Result<Vec<Member>, ApiError> {
    ctx.require_admin()?;
    guard_last_owner(db, ctx, user_id, true).await?;

    if !repository::remove_member(db, ctx.organization_id, user_id).await? {
        return Err(ApiError::NotFound("that user is not a member".to_string()));
    }

    members(db, ctx).await
}

/// Refuses a change that would leave the organization with no owner.
///
/// Only an owner can appoint another, so an organization that loses its last
/// one becomes permanently unmanageable. The database cannot express this, so
/// the service does.
async fn guard_last_owner(
    db: &PgPool,
    ctx: &OrgContext,
    user_id: Uuid,
    losing_ownership: bool,
) -> Result<(), ApiError> {
    if !losing_ownership {
        return Ok(());
    }

    let target_is_owner = repository::members(db, ctx.organization_id)
        .await?
        .into_iter()
        .any(|m| m.user_id == user_id && m.role == "OWNER");

    if target_is_owner && repository::owner_count(db, ctx.organization_id).await? <= 1 {
        return Err(ApiError::Conflict(
            "an organization must keep at least one owner".to_string(),
        ));
    }

    Ok(())
}
