use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// A deployment target: Development, Staging, Production and so on.
///
/// Every organization gets the standard four when it is created; these
/// endpoints exist because real landscapes have more, and different names.
#[derive(Debug, Serialize, ToSchema)]
pub struct Environment {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    /// Marks the environment whose incidents actually matter most. Used for
    /// ordering and, later, for alerting policy.
    pub is_production: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
