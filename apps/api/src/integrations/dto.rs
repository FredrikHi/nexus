use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::model::{Criticality, IntegrationStatus};

/// Request body for POST /integrations.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateIntegration {
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub source_component_id: Uuid,
    pub destination_component_id: Uuid,
    pub integration_type_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub owner_team_id: Option<Uuid>,
    #[serde(default = "default_criticality")]
    pub criticality: Criticality,
    #[serde(default = "default_status")]
    pub status: IntegrationStatus,
    pub documentation_url: Option<String>,
    #[serde(default = "default_true")]
    pub monitoring_enabled: bool,
    #[serde(default)]
    pub health_check_enabled: bool,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

fn default_criticality() -> Criticality {
    Criticality::Medium
}
fn default_status() -> IntegrationStatus {
    IntegrationStatus::Active
}
fn default_true() -> bool {
    true
}

/// Request body for PATCH /integrations/{id}. `None` means "leave unchanged".
///
/// Unlike a component's parent system, the endpoints of an integration may be
/// re-pointed: rewiring an existing edge is a normal operation. Any reference
/// supplied here is re-validated before the update runs.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateIntegration {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub source_component_id: Option<Uuid>,
    pub destination_component_id: Option<Uuid>,
    pub integration_type_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub owner_team_id: Option<Uuid>,
    pub criticality: Option<Criticality>,
    pub status: Option<IntegrationStatus>,
    pub documentation_url: Option<String>,
    pub monitoring_enabled: Option<bool>,
    pub health_check_enabled: Option<bool>,
    pub metadata: Option<serde_json::Value>,
}

/// Query string for GET /integrations.
///
/// `component_id` matches an integration touching that component at *either*
/// end, which is the question an operator actually asks: "what talks to this?"
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListIntegrationsQuery {
    pub environment_id: Option<Uuid>,
    pub component_id: Option<Uuid>,
}
