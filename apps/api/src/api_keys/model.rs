use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// An API key as anyone is allowed to see it: never the token itself.
///
/// `prefix` is the leading, non-secret slice of the token, kept so a human can
/// tell two keys apart in a list without the server ever storing the secret.
#[derive(Debug, Serialize, ToSchema)]
pub struct ApiKey {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub prefix: String,
    pub environment_id: Option<Uuid>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Returned exactly once, from the create call.
///
/// `token` is the only time the plaintext exists outside the caller's process.
/// It is not recoverable afterwards: only a SHA-256 hash of it is stored, so a
/// lost key is replaced rather than looked up.
#[derive(Debug, Serialize, ToSchema)]
pub struct CreatedApiKey {
    #[serde(flatten)]
    pub key: ApiKey,
    /// Shown once. Store it now; it cannot be retrieved again.
    pub token: String,
}

/// Who an authenticated ingest request belongs to.
///
/// Produced by the `ApiKeyAuth` extractor and consumed by the telemetry
/// service. This is the type that finally replaces `default_org_id()` on the
/// write path: the organization comes from a credential, not a constant.
#[derive(Debug, Clone, Copy)]
pub struct AuthenticatedKey {
    pub api_key_id: Uuid,
    pub organization_id: Uuid,
    /// When set, the key may only write events for this environment.
    pub environment_id: Option<Uuid>,
}
