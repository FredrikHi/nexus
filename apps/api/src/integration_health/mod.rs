mod dto;
mod evaluator;
mod handlers;
mod model;
mod repository;
mod service;
mod worker;

#[cfg(test)]
mod tests;

use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;

use crate::AppState;

// The verdict type is part of this feature's public surface: incidents are
// opened from it.
pub use model::HealthStatus;

/// Evaluates one organization. Exposed only for tests, so the incident suite
/// can drive a full pass exactly as the worker does.
#[cfg(test)]
pub use service::evaluate as evaluate_org;
pub use worker::{enabled as worker_enabled, spawn as spawn_worker, WorkerConfig};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/integration-health", get(handlers::list))
        .route("/api/v1/integration-health/overview", get(handlers::overview))
        .route("/api/v1/integration-health/evaluate", post(handlers::evaluate))
        .route("/api/v1/integration-health/{integration_id}", get(handlers::get))
        .route(
            "/api/v1/integration-health/{integration_id}/transitions",
            get(handlers::transitions),
        )
}

#[derive(OpenApi)]
#[openapi(paths(
    handlers::list,
    handlers::overview,
    handlers::get,
    handlers::transitions,
    handlers::evaluate,
))]
struct IntegrationHealthApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    IntegrationHealthApi::openapi()
}

/// The recorded health status of one integration as a raw token. A narrow
/// helper so a test can assert on health without importing the whole model.
#[cfg(test)]
pub async fn health_of(
    db: &sqlx::PgPool,
    integration_id: uuid::Uuid,
) -> Result<String, crate::error::ApiError> {
    Ok(service::get(db, integration_id).await?.status.as_str().to_string())
}
