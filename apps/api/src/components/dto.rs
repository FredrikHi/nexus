use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::model::ComponentType;

/// Request body for POST /components. A DTO, not the domain model: no id or
/// timestamps, `slug` optional (derived from `name` when absent), defaults.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateComponent {
    pub system_id: Uuid,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    #[serde(default = "default_component_type")]
    pub component_type: ComponentType,
    pub owner_team_id: Option<Uuid>,
    pub repository_url: Option<String>,
    pub documentation_url: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

fn default_component_type() -> ComponentType {
    ComponentType::Other
}

/// Request body for PATCH /components/{id}. Every field optional: `None` means
/// "leave unchanged", `Some(..)` sets it.
///
/// `system_id` is deliberately absent: moving a component to another system
/// would change its uniqueness scope and its integrations' meaning, so that is
/// a separate operation rather than a field of a partial update.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateComponent {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub component_type: Option<ComponentType>,
    pub owner_team_id: Option<Uuid>,
    pub repository_url: Option<String>,
    pub documentation_url: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Query string for GET /components, e.g. `?system_id=<uuid>`.
/// Axum's `Query` extractor deserializes this; a malformed UUID yields 400.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListComponentsQuery {
    pub system_id: Option<Uuid>,
}
