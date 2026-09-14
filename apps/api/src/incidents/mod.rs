mod dto;
mod handlers;
mod model;
mod repository;
mod service;

#[cfg(test)]
mod tests;

use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;

use crate::AppState;

// The health worker runs this after each evaluation pass.
pub use service::reconcile_all;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/incidents", get(handlers::list))
        .route("/api/v1/incidents/reconcile", post(handlers::reconcile))
        .route("/api/v1/incidents/{id}", get(handlers::get))
        .route(
            "/api/v1/incidents/{id}/acknowledge",
            post(handlers::acknowledge),
        )
        .route("/api/v1/incidents/{id}/notes", post(handlers::add_note))
}

#[derive(OpenApi)]
#[openapi(paths(
    handlers::list,
    handlers::get,
    handlers::acknowledge,
    handlers::add_note,
    handlers::reconcile,
))]
struct IncidentsApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    IncidentsApi::openapi()
}
