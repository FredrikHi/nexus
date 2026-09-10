use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::model::{Criticality, LifecycleStatus, SystemType};

/// Request body for POST /systems. This is a DTO, not the domain model:
/// no id/timestamps, slug is optional (derived from name if absent), and
/// some fields have defaults.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSystem {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub system_type: SystemType,
    pub owner_team_id: Option<Uuid>,
    pub documentation_url: Option<String>,
    pub repository_url: Option<String>,
    #[serde(default = "default_criticality")]
    pub criticality: Criticality,
    #[serde(default = "default_lifecycle")]
    pub lifecycle_status: LifecycleStatus,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

fn default_criticality() -> Criticality {
    Criticality::Medium
}
fn default_lifecycle() -> LifecycleStatus {
    LifecycleStatus::Active
}

/// Request body for PATCH /systems/{id}. Every field optional: `None` means
/// "leave unchanged". (`Some(...)` sets it.)
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSystem {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub system_type: Option<SystemType>,
    pub owner_team_id: Option<Uuid>,
    pub documentation_url: Option<String>,
    pub repository_url: Option<String>,
    pub criticality: Option<Criticality>,
    pub lifecycle_status: Option<LifecycleStatus>,
    pub metadata: Option<serde_json::Value>,
}
