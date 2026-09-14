use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::dto::CreateApiKey;
use super::model::{ApiKey, AuthenticatedKey, CreatedApiKey};
use super::repository;
use super::token;

pub async fn list(db: &PgPool, org_id: Uuid) -> Result<Vec<ApiKey>, ApiError> {
    repository::list_by_org(db, org_id).await
}

pub async fn create(
    db: &PgPool,
    org_id: Uuid,
    input: CreateApiKey,
) -> Result<CreatedApiKey, ApiError> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(ApiError::Validation("name must not be empty".to_string()));
    }

    if let Some(expires_at) = input.expires_at {
        if expires_at <= chrono::Utc::now() {
            return Err(ApiError::Validation(
                "expires_at must be in the future".to_string(),
            ));
        }
    }

    // A failure here means the OS randomness source is unavailable. Surfacing
    // it as a 500 is correct: issuing a predictable credential would be worse
    // than issuing none at all.
    let generated = token::generate()
        .map_err(|e| ApiError::Internal(format!("could not generate a token: {e}")))?;

    let key = repository::insert(
        db,
        org_id,
        repository::NewApiKey {
            name,
            prefix: &generated.prefix,
            token_hash: &generated.token_hash,
            environment_id: input.environment_id,
            expires_at: input.expires_at,
            allow_auto_create: input.allow_auto_create,
        },
    )
    .await?;

    // The only moment the plaintext leaves this process.
    Ok(CreatedApiKey {
        key,
        token: generated.token,
    })
}

pub async fn revoke(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<ApiKey, ApiError> {
    repository::revoke(db, org_id, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("no active api key {id}")))
}

/// Turns a presented token into its owner, or fails with 401.
///
/// Every failure mode returns the same message on purpose. Telling a caller
/// whether a token is unknown, revoked or merely expired hands an attacker a
/// probe they should not have.
pub async fn authenticate(db: &PgPool, presented: &str) -> Result<AuthenticatedKey, ApiError> {
    let hash = token::hash(presented);

    let key = repository::authenticate(db, &hash)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("invalid or expired API key".to_string()))?;

    // Best effort: failing to record usage must not fail the request.
    if let Err(err) = repository::touch_last_used(db, key.api_key_id).await {
        tracing::warn!(error = %err, "could not record api key usage");
    }

    Ok(key)
}
