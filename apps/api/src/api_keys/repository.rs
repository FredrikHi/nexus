use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::model::{ApiKey, AuthenticatedKey};

pub async fn insert(
    db: &PgPool,
    org_id: Uuid,
    name: &str,
    prefix: &str,
    token_hash: &str,
    environment_id: Option<Uuid>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<ApiKey, ApiError> {
    let key = sqlx::query_as!(
        ApiKey,
        r#"INSERT INTO api_keys
             (organization_id, name, prefix, token_hash, environment_id, expires_at)
           VALUES ($1,$2,$3,$4,$5,$6)
           RETURNING id, organization_id, name, prefix, environment_id,
                     last_used_at, expires_at, revoked_at, created_at, updated_at"#,
        org_id,
        name,
        prefix,
        token_hash,
        environment_id,
        expires_at
    )
    .fetch_one(db)
    .await?;

    Ok(key)
}

pub async fn list_by_org(db: &PgPool, org_id: Uuid) -> Result<Vec<ApiKey>, ApiError> {
    let keys = sqlx::query_as!(
        ApiKey,
        r#"SELECT id, organization_id, name, prefix, environment_id,
                  last_used_at, expires_at, revoked_at, created_at, updated_at
           FROM api_keys
           WHERE organization_id = $1
           ORDER BY created_at DESC"#,
        org_id
    )
    .fetch_all(db)
    .await?;

    Ok(keys)
}

/// Marks a key unusable. The row is kept on purpose: for a credential, the
/// record that it existed and when it stopped working is the audit trail.
/// Returns None when there is no such live key in this organization.
pub async fn revoke(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<Option<ApiKey>, ApiError> {
    let key = sqlx::query_as!(
        ApiKey,
        r#"UPDATE api_keys
           SET revoked_at = now()
           WHERE id = $1 AND organization_id = $2 AND revoked_at IS NULL
           RETURNING id, organization_id, name, prefix, environment_id,
                     last_used_at, expires_at, revoked_at, created_at, updated_at"#,
        id,
        org_id
    )
    .fetch_optional(db)
    .await?;

    Ok(key)
}

/// Resolves a presented token hash to its owner.
///
/// The lookup is by hash, so the plaintext is never compared and a timing
/// attack has nothing to measure: an unknown hash simply misses the index.
/// Revoked and expired keys are filtered in SQL rather than in Rust, so there
/// is no window where a caller forgets to check.
pub async fn authenticate(
    db: &PgPool,
    token_hash: &str,
) -> Result<Option<AuthenticatedKey>, ApiError> {
    let found = sqlx::query_as!(
        AuthenticatedKey,
        r#"SELECT id AS "api_key_id!", organization_id AS "organization_id!", environment_id
           FROM api_keys
           WHERE token_hash = $1
             AND revoked_at IS NULL
             AND (expires_at IS NULL OR expires_at > now())"#,
        token_hash
    )
    .fetch_optional(db)
    .await?;

    Ok(found)
}

/// Records that a key was used, at most once a minute.
///
/// The guard matters: without it every ingest request would write a row, and
/// ingest is the highest-volume path in the system.
pub async fn touch_last_used(db: &PgPool, id: Uuid) -> Result<(), ApiError> {
    sqlx::query!(
        r#"UPDATE api_keys
           SET last_used_at = now()
           WHERE id = $1
             AND (last_used_at IS NULL OR last_used_at < now() - INTERVAL '1 minute')"#,
        id
    )
    .execute(db)
    .await?;

    Ok(())
}
