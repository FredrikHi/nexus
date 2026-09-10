use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTeam {
    pub name: String,
    /// Derived from the name when absent.
    pub slug: Option<String>,
    pub description: Option<String>,
}

/// Every field optional: `None` leaves it unchanged.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTeam {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
}

/// Adds someone who is already a member of the organization.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddTeamMember {
    pub user_id: Uuid,
}
