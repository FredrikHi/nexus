use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::{Membership, OrgRole};
use crate::error::ApiError;

#[derive(Debug, Serialize, ToSchema)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// One person in an organization.
#[derive(Debug, Serialize, ToSchema)]
pub struct Member {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: OrgRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct MemberRow {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub role: String,
    pub joined_at: DateTime<Utc>,
}

impl MemberRow {
    pub fn into_domain(self) -> Result<Member, ApiError> {
        let role = OrgRole::from_db(&self.role)
            .ok_or_else(|| ApiError::Internal(format!("unknown role '{}'", self.role)))?;

        Ok(Member {
            user_id: self.user_id,
            email: self.email,
            display_name: self.display_name,
            avatar_url: self.avatar_url,
            role,
            joined_at: self.joined_at,
        })
    }
}

/// Answer to "who am I and where do I belong", which is the first call a
/// freshly signed-in client makes.
#[derive(Debug, Serialize, ToSchema)]
pub struct Me {
    pub user_id: Uuid,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub organizations: Vec<Membership>,
}
