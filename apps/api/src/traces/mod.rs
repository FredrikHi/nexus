mod dto;
mod handlers;
mod model;
mod repository;
mod service;

#[cfg(test)]
mod tests;

use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/traces", get(handlers::list))
        .route("/api/v1/traces/{trace_id}", get(handlers::get))
}

#[derive(OpenApi)]
#[openapi(paths(handlers::list, handlers::get))]
struct TracesApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    TracesApi::openapi()
}
