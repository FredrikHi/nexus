use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// Request body for POST /api-keys.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiKey {
    /// Human label, unique within the organization, e.g. "billing-worker-prod".
    pub name: String,
    /// Optional: pin the key to one environment so a staging agent cannot
    /// write events claiming to be production.
    pub environment_id: Option<Uuid>,
    /// Optional expiry. A key with no expiry is valid until revoked.
    pub expires_at: Option<DateTime<Utc>>,
}
