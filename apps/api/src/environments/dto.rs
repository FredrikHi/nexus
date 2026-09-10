use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateEnvironment {
    pub name: String,
    /// Derived from the name when absent.
    pub slug: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub is_production: bool,
}

/// Every field optional: `None` leaves it unchanged.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateEnvironment {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub is_production: Option<bool>,
}
