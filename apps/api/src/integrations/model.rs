use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::ApiError;

// Shared with `systems`; see crate::domain.
pub use crate::domain::Criticality;

/// Mirrors the `status` TEXT + CHECK column on `integrations`.
///
/// Deliberately not the same type as a system's `LifecycleStatus`: the token
/// sets differ, and collapsing them would let a nonsense value type-check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IntegrationStatus {
    Active,
    Inactive,
    Deprecated,
}

impl IntegrationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            IntegrationStatus::Active => "ACTIVE",
            IntegrationStatus::Inactive => "INACTIVE",
            IntegrationStatus::Deprecated => "DEPRECATED",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "ACTIVE" => Some(IntegrationStatus::Active),
            "INACTIVE" => Some(IntegrationStatus::Inactive),
            "DEPRECATED" => Some(IntegrationStatus::Deprecated),
            _ => None,
        }
    }
}

/// The domain model: one directed edge from a source component to a
/// destination component, of a given integration type, in one environment.
/// A directed edge from a source component to a destination component.
#[derive(Debug, Serialize, ToSchema)]
pub struct Integration {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub source_component_id: Uuid,
    pub destination_component_id: Uuid,
    pub integration_type_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub owner_team_id: Option<Uuid>,
    pub criticality: Criticality,
    pub status: IntegrationStatus,
    pub documentation_url: Option<String>,
    pub monitoring_enabled: bool,
    pub health_check_enabled: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The raw DB row: enum columns arrive as `String`. `into_domain` is the one
/// place strings become typed enums. `query_as!` constructs it directly, so no
/// `FromRow` derive is needed.
#[derive(Debug)]
pub struct IntegrationRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub source_component_id: Uuid,
    pub destination_component_id: Uuid,
    pub integration_type_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub owner_team_id: Option<Uuid>,
    pub criticality: String,
    pub status: String,
    pub documentation_url: Option<String>,
    pub monitoring_enabled: bool,
    pub health_check_enabled: bool,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl IntegrationRow {
    pub fn into_domain(self) -> Result<Integration, ApiError> {
        // CHECK constraints should make these unreachable; an unknown token is
        // a bug on our side, so it becomes a 500 rather than a panic.
        let criticality = Criticality::from_db(&self.criticality).ok_or_else(|| {
            ApiError::Internal(format!("unknown criticality '{}'", self.criticality))
        })?;
        let status = IntegrationStatus::from_db(&self.status)
            .ok_or_else(|| ApiError::Internal(format!("unknown status '{}'", self.status)))?;

        Ok(Integration {
            id: self.id,
            organization_id: self.organization_id,
            name: self.name,
            slug: self.slug,
            description: self.description,
            source_component_id: self.source_component_id,
            destination_component_id: self.destination_component_id,
            integration_type_id: self.integration_type_id,
            environment_id: self.environment_id,
            owner_team_id: self.owner_team_id,
            criticality,
            status,
            documentation_url: self.documentation_url,
            monitoring_enabled: self.monitoring_enabled,
            health_check_enabled: self.health_check_enabled,
            metadata: self.metadata,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}

/// Result of the single round trip that checks every referenced id at once.
/// Not a table row: `query_as!` maps any result set whose column names line up
/// with the field names, not just a `SELECT` from one table.
#[derive(Debug)]
pub struct ReferenceCheck {
    pub source_exists: bool,
    pub destination_exists: bool,
    pub integration_type_exists: bool,
    pub environment_exists: bool,
}
