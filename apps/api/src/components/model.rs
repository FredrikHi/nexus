use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::ApiError;

/// Mirrors the `component_type` TEXT + CHECK column. serde uses the exact DB
/// tokens, so an unknown value in a request body fails deserialization and
/// Axum answers 422 without us writing any validation code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ComponentType {
    Frontend,
    Api,
    Controller,
    Service,
    Repository,
    Database,
    StoredProcedure,
    Worker,
    Job,
    Queue,
    Webhook,
    Other,
}

impl ComponentType {
    pub fn as_str(self) -> &'static str {
        match self {
            ComponentType::Frontend => "FRONTEND",
            ComponentType::Api => "API",
            ComponentType::Controller => "CONTROLLER",
            ComponentType::Service => "SERVICE",
            ComponentType::Repository => "REPOSITORY",
            ComponentType::Database => "DATABASE",
            ComponentType::StoredProcedure => "STORED_PROCEDURE",
            ComponentType::Worker => "WORKER",
            ComponentType::Job => "JOB",
            ComponentType::Queue => "QUEUE",
            ComponentType::Webhook => "WEBHOOK",
            ComponentType::Other => "OTHER",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "FRONTEND" => Some(ComponentType::Frontend),
            "API" => Some(ComponentType::Api),
            "CONTROLLER" => Some(ComponentType::Controller),
            "SERVICE" => Some(ComponentType::Service),
            "REPOSITORY" => Some(ComponentType::Repository),
            "DATABASE" => Some(ComponentType::Database),
            "STORED_PROCEDURE" => Some(ComponentType::StoredProcedure),
            "WORKER" => Some(ComponentType::Worker),
            "JOB" => Some(ComponentType::Job),
            "QUEUE" => Some(ComponentType::Queue),
            "WEBHOOK" => Some(ComponentType::Webhook),
            "OTHER" => Some(ComponentType::Other),
            _ => None,
        }
    }
}

/// The domain model: what the rest of the app and the API response see.
/// A component always belongs to exactly one system.
/// A component: a part inside a system, and the thing integrations connect.
#[derive(Debug, Serialize, ToSchema)]
pub struct Component {
    pub id: Uuid,
    pub system_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub component_type: ComponentType,
    pub owner_team_id: Option<Uuid>,
    pub repository_url: Option<String>,
    pub documentation_url: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The raw DB row: the enum column arrives as `String`. `into_domain` is the
/// single place where a string becomes a typed enum. `query_as!` constructs it
/// directly, so no `FromRow` derive is needed.
#[derive(Debug)]
pub struct ComponentRow {
    pub id: Uuid,
    pub system_id: Uuid,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub component_type: String,
    pub owner_team_id: Option<Uuid>,
    pub repository_url: Option<String>,
    pub documentation_url: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ComponentRow {
    pub fn into_domain(self) -> Result<Component, ApiError> {
        // The CHECK constraint should make this unreachable; if a row somehow
        // holds an unknown token that's a bug on our side, so it becomes a 500
        // rather than a panic.
        let component_type = ComponentType::from_db(&self.component_type).ok_or_else(|| {
            ApiError::Internal(format!("unknown component_type '{}'", self.component_type))
        })?;

        Ok(Component {
            id: self.id,
            system_id: self.system_id,
            name: self.name,
            slug: self.slug,
            description: self.description,
            component_type,
            owner_team_id: self.owner_team_id,
            repository_url: self.repository_url,
            documentation_url: self.documentation_url,
            metadata: self.metadata,
            created_at: self.created_at,
            updated_at: self.updated_at,
        })
    }
}
