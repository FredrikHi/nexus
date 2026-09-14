//! Extractors that turn a bearer token into a caller, and a caller into a
//! tenant-scoped context.
//!
//! This is what finally replaces `default_org_id()`. A handler that takes
//! `OrgContext` cannot forget to scope its work, because the organization
//! arrives as a value it has to pass on rather than a constant it could ignore.

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use jsonwebtoken::{decode, decode_header, Validation};
use uuid::Uuid;

use crate::error::ApiError;
use crate::AppState;

use super::claims::Claims;
use super::model::{Identity, OrgContext, OrgRole};
use super::repository;

/// Header a client uses to say which organization a request is for.
pub const ORG_HEADER: &str = "x-organization-id";

/// A verified caller, with no organization attached yet.
///
/// Used by the few endpoints that are about the person rather than a tenant:
/// "who am I", "which organizations am I in", "create an organization".
pub struct Authenticated(pub Identity);

impl FromRequestParts<AppState> for Authenticated {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let identity = authenticate(parts, state).await?;
        Ok(Authenticated(identity))
    }
}

impl FromRequestParts<AppState> for OrgContext {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let identity = authenticate(parts, state).await?;

        let organization_id = match organization_header(parts)? {
            Some(id) => id,
            // No header: assume the organization only when there is exactly
            // one possibility. Guessing between two would silently write into
            // the wrong tenant, which is worse than making the client say.
            None => {
                let candidates =
                    repository::first_two_memberships(&state.db, identity.user_id).await?;
                match candidates.len() {
                    1 => candidates[0],
                    0 => {
                        return Err(ApiError::BadRequest(
                            "you do not belong to any organization yet; create one first"
                                .to_string(),
                        ))
                    }
                    _ => {
                        return Err(ApiError::BadRequest(format!(
                            "you belong to more than one organization; send the {ORG_HEADER} header to say which one"
                        )))
                    }
                }
            }
        };

        // The token proves who you are. This proves you may act here.
        let role = repository::membership(&state.db, identity.user_id, organization_id)
            .await?
            .ok_or_else(|| {
                // Deliberately the same answer whether the organization does
                // not exist or the caller is simply not in it: otherwise this
                // endpoint enumerates other people's organizations.
                ApiError::Forbidden("you are not a member of that organization".to_string())
            })?;

        let role = OrgRole::from_db(&role)
            .ok_or_else(|| ApiError::Internal(format!("unknown role '{role}'")))?;

        Ok(OrgContext {
            identity,
            organization_id,
            role,
        })
    }
}

/// Verifies the bearer token and resolves it to a local user row.
async fn authenticate(parts: &Parts, state: &AppState) -> Result<Identity, ApiError> {
    let token = bearer_token(parts)
        .ok_or_else(|| ApiError::Unauthorized("missing bearer token".to_string()))?;

    // The key id in the header says which key signed this, which is what makes
    // rotation possible: the auth service can publish a new key and start
    // using it without any coordinated restart here.
    let header =
        decode_header(&token).map_err(|_| ApiError::Unauthorized("malformed token".to_string()))?;
    let kid = header
        .kid
        .ok_or_else(|| ApiError::Unauthorized("token has no key id".to_string()))?;

    let key = state.jwks.key_for(&kid).await?;

    let mut validation = Validation::new(header.alg);
    validation.set_audience(&[state.auth_audience.as_str()]);
    validation.set_issuer(&[state.auth_issuer.as_str()]);
    validation.set_required_spec_claims(&["exp", "sub", "aud", "iss"]);

    let claims = decode::<Claims>(&token, &key, &validation)
        .map_err(|e| ApiError::Unauthorized(format!("token rejected: {e}")))?
        .claims;

    // First sight of an account creates the local row; later sights refresh
    // the profile, so a changed name or avatar follows without a sync job.
    let display_name = claims.name.clone().unwrap_or_else(|| claims.email.clone());

    repository::upsert_by_subject(
        &state.db,
        &claims.sub,
        &claims.email,
        &display_name,
        claims.picture.as_deref(),
    )
    .await
}

fn bearer_token(parts: &Parts) -> Option<String> {
    let value = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    // The scheme is case-insensitive per RFC 7235; the token is not.
    if scheme.eq_ignore_ascii_case("bearer") && !token.trim().is_empty() {
        Some(token.trim().to_string())
    } else {
        None
    }
}

fn organization_header(parts: &Parts) -> Result<Option<Uuid>, ApiError> {
    let Some(value) = parts.headers.get(ORG_HEADER) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| ApiError::BadRequest(format!("{ORG_HEADER} is not valid text")))?;

    Uuid::parse_str(value.trim())
        .map(Some)
        .map_err(|_| ApiError::BadRequest(format!("{ORG_HEADER} is not a valid UUID")))
}
