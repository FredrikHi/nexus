use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// A group of people inside an organization, and the unit that owns systems,
/// components and integrations.
///
/// Distinct from organization membership: membership says what you may do,
/// a team says what you are responsible for.
#[derive(Debug, Serialize, ToSchema)]
pub struct Team {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    /// How many people are in it, so a list needs no second call.
    pub member_count: i64,
    /// How many systems name this team as their owner.
    pub owned_systems: i64,
    /// How many integrations name this team as their owner.
    pub owned_integrations: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Someone in a team. Their authority comes from organization membership, not
/// from the team, so no role appears here.
#[derive(Debug, Serialize, ToSchema)]
pub struct TeamMember {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}
