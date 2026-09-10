use serde::Deserialize;
use utoipa::ToSchema;

use crate::auth::OrgRole;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateOrganization {
    pub name: String,
    /// Derived from the name when absent. Unique across the whole platform,
    /// because it addresses the organization in a URL.
    pub slug: Option<String>,
    pub description: Option<String>,
}

/// Adds someone who has already signed in at least once.
///
/// There is no invitation flow yet: an account must exist before it can be
/// added, which means the person signs in first and is then granted access.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddMember {
    pub email: String,
    #[serde(default = "default_role")]
    pub role: OrgRole,
}

fn default_role() -> OrgRole {
    OrgRole::Viewer
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMemberRole {
    pub role: OrgRole,
}
