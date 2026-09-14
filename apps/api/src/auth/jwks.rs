//! Fetching and caching the auth service's public keys.
//!
//! The API never holds a signing key and never talks to the auth service on
//! the request path. It fetches that service's JSON Web Key Set once, caches
//! it, and verifies every token locally. Validation therefore costs no network
//! round trip, and the auth service being briefly down does not stop anyone
//! using an already-issued token.
//!
//! The trade-off, accepted when this design was chosen: revocation waits for
//! the token to expire, so tokens must be short-lived.

use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::DecodingKey;
use tokio::sync::RwLock;

use crate::error::ApiError;

/// How long a fetched key set is trusted before refetching.
const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
/// Shortest gap between refetches triggered by an unknown key id, so a stream
/// of bogus tokens cannot turn into a stream of requests to the auth service.
const MIN_REFETCH_INTERVAL: Duration = Duration::from_secs(30);

struct Cached {
    keys: JwkSet,
    fetched_at: Instant,
}

/// Shared, cheap to clone: the inner state is behind an Arc.
#[derive(Clone)]
pub struct JwksCache {
    url: String,
    http: reqwest::Client,
    cached: Arc<RwLock<Option<Cached>>>,
}

impl JwksCache {
    pub fn new(url: String) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        JwksCache {
            url,
            http,
            cached: Arc::new(RwLock::new(None)),
        }
    }

    /// Resolves a key id to a verification key.
    ///
    /// An unknown `kid` triggers one refetch, because that is what key
    /// rotation looks like from here: the auth service starts signing with a
    /// key we have never seen. The rate limit stops that becoming a
    /// denial-of-service amplifier for anyone sending junk tokens.
    pub async fn key_for(&self, kid: &str) -> Result<DecodingKey, ApiError> {
        if let Some(key) = self.lookup(kid).await? {
            return Ok(key);
        }

        self.refresh(false).await?;

        self.lookup(kid)
            .await?
            .ok_or_else(|| ApiError::Unauthorized("token was signed by an unknown key".to_string()))
    }

    /// Looks in the cache, refreshing first if it has gone stale.
    async fn lookup(&self, kid: &str) -> Result<Option<DecodingKey>, ApiError> {
        let stale = {
            let guard = self.cached.read().await;
            match guard.as_ref() {
                None => true,
                Some(c) => c.fetched_at.elapsed() > CACHE_TTL,
            }
        };
        if stale {
            self.refresh(true).await?;
        }

        let guard = self.cached.read().await;
        let Some(cached) = guard.as_ref() else {
            return Ok(None);
        };

        match cached.keys.find(kid) {
            // from_jwk handles every key type the auth service might use, so
            // switching it from Ed25519 to RSA needs no change here.
            Some(jwk) => DecodingKey::from_jwk(jwk)
                .map(Some)
                .map_err(|e| ApiError::Internal(format!("unusable key in the JWKS: {e}"))),
            None => Ok(None),
        }
    }

    /// Refetches the key set. `force` skips the rate limit, and is used when
    /// the cache is empty or expired rather than merely missing a key id.
    async fn refresh(&self, force: bool) -> Result<(), ApiError> {
        if !force {
            let guard = self.cached.read().await;
            if let Some(c) = guard.as_ref() {
                if c.fetched_at.elapsed() < MIN_REFETCH_INTERVAL {
                    return Ok(());
                }
            }
        }

        let response =
            self.http.get(&self.url).send().await.map_err(|e| {
                ApiError::Internal(format!("could not reach the auth service: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(ApiError::Internal(format!(
                "auth service returned {} for its key set",
                response.status()
            )));
        }

        let keys: JwkSet = response
            .json()
            .await
            .map_err(|e| ApiError::Internal(format!("malformed key set: {e}")))?;

        let mut guard = self.cached.write().await;
        *guard = Some(Cached {
            keys,
            fetched_at: Instant::now(),
        });
        Ok(())
    }

    /// Seeds the cache directly. Used by tests so they can verify real tokens
    /// without an auth service running.
    #[cfg(test)]
    pub async fn seed(&self, keys: JwkSet) {
        let mut guard = self.cached.write().await;
        *guard = Some(Cached {
            keys,
            fetched_at: Instant::now(),
        });
    }
}
