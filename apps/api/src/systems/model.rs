use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::ApiError;

// Fixed value-sets mirrored from the TEXT + CHECK columns.
// serde (de)serializes them as the exact DB tokens (SCREAMING_SNAKE_CASE), so
// an unknown value in a request body fails to deserialize -> 422, for free.
// `as_str`/`from_db` bridge the enum to the TEXT column in SQL.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SystemType {
    Application,
    Api,
    Database,
    ExternalService,
    MessageBroker,
    IdentityProvider,
    FileService,
    Other,
}

impl SystemType {
    pub fn as_str(self) -> &'static str {
        match self {
            SystemType::Application => "APPLICATION",
            SystemType::Api => "API",
            SystemType::Database => "DATABASE",
            SystemType::ExternalService => "EXTERNAL_SERVICE",
            SystemType::MessageBroker => "MESSAGE_BROKER",
            SystemType::IdentityProvider => "IDENTITY_PROVIDER",
            SystemType::FileService => "FILE_SERVICE",
            SystemType::Other => "OTHER",
        }
    }
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "APPLICATION" => Some(SystemType::Application),
            "API" => Some(SystemType::Api),
            "DATABASE" => Some(SystemType::Database),
            "EXTERNAL_SERVICE" => Some(SystemType::ExternalService),
            "MESSAGE_BROKER" => Some(SystemType::MessageBroker),
            "IDENTITY_PROVIDER" => Some(SystemType::IdentityProvider),
            "FILE_SERVICE" => Some(SystemType::FileService),
            "OTHER" => Some(SystemType::Other),
            _ => None,
        }
    }
}

// Criticality is shared with `integrations`, so it lives in `crate::domain`.
// Re-exported here so `super::model::Criticality` keeps resolving inside this
// feature, exactly as if it were still declared in this file.
pub use crate::domain::Criticality;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LifecycleStatus {
    Planned,
    Active,
    Deprecated,
    Retired,
}

impl LifecycleStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            LifecycleStatus::Planned => "PLANNED",
            LifecycleStatus::Active => "ACTIVE",
            LifecycleStatus::Deprecated => "DEPRECATED",
            LifecycleStatus::Retired => "RETIRED",
        }
    }
    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "PLANNED" => Some(LifecycleStatus::Planned),
            "ACTIVE" => Some(LifecycleStatus::Active),
            "DEPRECATED" => Some(LifecycleStatus::Deprecated),
            "RETIRED" => Some(LifecycleStatus::Retired),
            _ => None,
        }
    }
}

/// The domain model: what the rest of the app (and the API response) sees.
/// Enums are real types here, not loose strings.
/// A system: a deployable unit or external service in the landscape.
#[derive(Debug, Serialize, ToSchema)]
pub struct System {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub system_type: SystemType,
    pub owner_team_id: Option<Uuid>,
    pub documentation_url: Option<String>,
    pub repository_url: Option<String>,
    pub criticality: Criticality,
    pub lifecycle_status: LifecycleStatus,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The raw DB row: enum columns arrive as `String`. Kept separate so the
/// repository speaks "database shape" and the rest of the app speaks "domain
/// shape". `into_domain` is the one place strings become typed enums.
///
/// `query_as!` constructs this directly by matching result columns to fields,
/// so no `FromRow` derive is needed.
#[derive(Debug)]
pub struct SystemRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub system_type: String,
    pub owner_team_id: Option<Uuid>,
    pub documentation_url: Option<String>,
    pub repository_url: Option<String>,
    pub criticality: String,
    pub lifecycle_status: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SystemRow {
    pub fn into_domain(self) -> Result<System, ApiError> {
        // The DB's CHECK constraints should make these always valid; if a row
        // somehow holds an unknown token, that's a bug, so it's a 500 (Internal)
        // rather than a panic. This is why we don't `.unwrap()` here.
        let system_type = SystemType::from_db(&self.system_type).ok_or_else(|| {
            ApiError::Internal(format!("unknown system_type '{}'", self.system_type))
        })?;
        let criticality = Criticality::from_db(&self.criticality).ok_or_else(|| {
            ApiError::Internal(format!("unknown criticality '{}'", self.criticality))
        })?;
        let lifecycle_status =
            LifecycleStatus::from_db(&self.lifecycle_status).ok_or_else(|| {
                ApiError::Internal(format!(
                    "unknown lifecycle_status '{}'",
                    self.lifecycle_status
                ))
            })?;

        Ok(System {
            id: self.id,
            organization_id: self.organization_id,
            name: self.name,
            slug: self.slug,
            description: self.description,
            system_type,
            owner_team_id: self.owner_team_id,
            documentation_url: self.documentation_url,
            repository_url: self.repository_url,
            criticality,
            lifecycle_status,
            metadata: self.metadata,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}
