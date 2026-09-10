//! The extractor that turns a credential into an organization.
//!
//! This is where multi-tenancy stops being a placeholder. A handler that takes
//! `ApiKeyAuth` cannot forget to scope its work, because the organization
//! arrives as a value it has to use rather than a constant it could ignore.

use axum::extract::FromRequestParts;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

use crate::error::ApiError;
use crate::AppState;

use super::model::AuthenticatedKey;
use super::service;

/// Header alternative to `Authorization: Bearer`, common in agent configs.
const API_KEY_HEADER: &str = "x-api-key";

/// An authenticated ingest caller.
pub struct ApiKeyAuth(pub AuthenticatedKey);

/// Implemented for `AppState` specifically rather than a generic `S`, because
/// resolving the credential needs the connection pool.
impl FromRequestParts<AppState> for ApiKeyAuth {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let presented = presented_token(parts).ok_or_else(|| {
            ApiError::Unauthorized(
                "missing API key: send Authorization: Bearer <token> or X-API-Key: <token>"
                    .to_string(),
            )
        })?;

        let key = service::authenticate(&state.db, &presented).await?;
        Ok(ApiKeyAuth(key))
    }
}

/// Reads the token from either accepted header, preferring the standard one.
fn presented_token(parts: &Parts) -> Option<String> {
    if let Some(value) = parts.headers.get(AUTHORIZATION) {
        let value = value.to_str().ok()?;
        // The scheme is case-insensitive per RFC 7235; the token is not.
        let (scheme, token) = value.split_once(' ')?;
        if scheme.eq_ignore_ascii_case("bearer") && !token.trim().is_empty() {
            return Some(token.trim().to_string());
        }
        return None;
    }

    let value = parts.headers.get(API_KEY_HEADER)?.to_str().ok()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
