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
