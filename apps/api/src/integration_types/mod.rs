mod dto;
mod handlers;
mod model;
mod repository;
mod service;

use axum::routing::{delete, get};
use axum::Router;
use utoipa::OpenApi;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/integration-types",
            get(handlers::list).post(handlers::create),
        )
        .route("/api/v1/integration-types/{id}", delete(handlers::delete))
}

#[derive(OpenApi)]
#[openapi(paths(handlers::list, handlers::create, handlers::delete))]
struct IntegrationTypesApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    IntegrationTypesApi::openapi()
}
