use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateIntegrationType {
    /// Stable token, e.g. "PEPPOL". Derived from the name when absent.
    pub key: Option<String>,
    pub name: String,
    pub description: Option<String>,
}
