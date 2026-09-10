use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// A kind of integration: HTTP, SFTP, a message queue, or something specific
/// to one organization such as a national e-invoicing network.
///
/// Unlike the fixed TEXT + CHECK value sets elsewhere, this is a real table
/// because the useful list is different in every landscape.
#[derive(Debug, Serialize, ToSchema)]
pub struct IntegrationType {
    pub id: Uuid,
    /// Stable token, unique among the built-ins or within one organization.
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    /// Built-ins ship with the platform and cannot be edited or removed.
    pub is_builtin: bool,
    /// None for a built-in. Set means the type belongs to that organization
    /// alone and is invisible to every other one.
    pub organization_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
