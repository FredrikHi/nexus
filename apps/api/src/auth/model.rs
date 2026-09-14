use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

/// Who the caller is, resolved from a verified token.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Identity {
    pub user_id: Uuid,
    /// The auth service's stable id for this account.
    pub subject: String,
    pub email: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

/// One organization the caller belongs to.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Membership {
    pub organization_id: Uuid,
    pub organization_name: String,
    pub organization_slug: String,
    pub role: OrgRole,
    pub joined_at: DateTime<Utc>,
}

/// The raw row; `role` arrives as TEXT.
#[derive(Debug)]
pub struct MembershipRow {
    pub organization_id: Uuid,
    pub organization_name: String,
    pub organization_slug: String,
    pub role: String,
    pub joined_at: DateTime<Utc>,
}

impl MembershipRow {
    pub fn into_domain(self) -> Result<Membership, crate::error::ApiError> {
        let role = OrgRole::from_db(&self.role).ok_or_else(|| {
            crate::error::ApiError::Internal(format!("unknown role '{}'", self.role))
        })?;

        Ok(Membership {
            organization_id: self.organization_id,
            organization_name: self.organization_name,
            organization_slug: self.organization_slug,
            role,
            joined_at: self.joined_at,
        })
    }
}

/// Authority within one organization.
///
/// OWNER is separate from ADMIN so an organization can never be left with
/// nobody able to manage it: the last owner cannot be removed or demoted.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, serde::Deserialize, ToSchema,
)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrgRole {
    Viewer,
    Operator,
    Editor,
    Admin,
    Owner,
}

impl OrgRole {
    pub fn as_str(self) -> &'static str {
        match self {
            OrgRole::Viewer => "VIEWER",
            OrgRole::Operator => "OPERATOR",
            OrgRole::Editor => "EDITOR",
            OrgRole::Admin => "ADMIN",
            OrgRole::Owner => "OWNER",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "VIEWER" => Some(OrgRole::Viewer),
            "OPERATOR" => Some(OrgRole::Operator),
            "EDITOR" => Some(OrgRole::Editor),
            "ADMIN" => Some(OrgRole::Admin),
            "OWNER" => Some(OrgRole::Owner),
            _ => None,
        }
    }

    /// May change domain data: systems, integrations, policies.
    pub fn can_write(self) -> bool {
        self >= OrgRole::Editor
    }

    /// May manage the organization itself: members, API keys.
    pub fn can_administer(self) -> bool {
        self >= OrgRole::Admin
    }
}

/// The caller plus the organization this request is acting in.
///
/// Handlers take this instead of calling a `default_org_id()` constant, so a
/// handler physically cannot forget to scope its work to a tenant.
#[derive(Debug, Clone)]
pub struct OrgContext {
    pub identity: Identity,
    pub organization_id: Uuid,
    pub role: OrgRole,
}

impl OrgContext {
    /// Rejects a caller who may read but not write.
    pub fn require_write(&self) -> Result<(), crate::error::ApiError> {
        if self.role.can_write() {
            Ok(())
        } else {
            Err(crate::error::ApiError::Forbidden(format!(
                "role {} may not modify this organization",
                self.role.as_str()
            )))
        }
    }

    /// Rejects a caller who may not manage the organization itself.
    pub fn require_admin(&self) -> Result<(), crate::error::ApiError> {
        if self.role.can_administer() {
            Ok(())
        } else {
            Err(crate::error::ApiError::Forbidden(format!(
                "role {} may not administer this organization",
                self.role.as_str()
            )))
        }
    }
}
