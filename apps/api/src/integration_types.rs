//! Read-only listing of the integration type catalogue.
//!
//! Unlike the fixed TEXT + CHECK value sets, `integration_types` is a real
//! table users may extend at runtime, so a client cannot know the valid ids
//! ahead of time. Without this endpoint the create-integration call is
//! unusable. Writes are deliberately out of scope for now.

use axum::extract::State;
use axum::routing::get;
use axum::Router;
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

use crate::auth::Authenticated;
use crate::error::ApiError;
use crate::extract::Json;
use crate::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct IntegrationType {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub is_builtin: bool,
    pub created_at: DateTime<Utc>,
}

#[utoipa::path(
    get,
    path = "/api/v1/integration-types",
    tag = "Reference data",
    responses(
        (status = 200, description = "The integration type catalogue", body = Vec<IntegrationType>),
    )
)]
async fn list(
    State(state): State<AppState>,
    // The catalogue is platform-wide rather than per-tenant, so it needs no
    // organization. It still requires a signed-in caller: the shape of an
    // installation is not public information.
    _caller: Authenticated,
) -> Result<Json<Vec<IntegrationType>>, ApiError> {
    let types = sqlx::query_as!(
        IntegrationType,
        r#"SELECT id, key, name, description, is_builtin, created_at
           FROM integration_types
           ORDER BY name"#
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(types))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/integration-types", get(list))
}

#[derive(OpenApi)]
#[openapi(paths(list))]
struct IntegrationTypesApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    IntegrationTypesApi::openapi()
}
