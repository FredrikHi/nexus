mod dto;
mod handlers;
mod model;
mod repository;
mod service;

use axum::routing::get;
use axum::Router;
use utoipa::OpenApi;

use crate::AppState;

/// Everything the feature exposes to the rest of the app: its routes.
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/integrations",
            get(handlers::list).post(handlers::create),
        )
        .route(
            "/api/v1/integrations/{id}",
            get(handlers::get).patch(handlers::update).delete(handlers::delete),
        )
}

/// The feature's slice of the OpenAPI document. Declared here so `handlers`
/// can stay private: a feature publishes its routes and its documentation.
#[derive(OpenApi)]
#[openapi(paths(
    handlers::list,
    handlers::get,
    handlers::create,
    handlers::update,
    handlers::delete,
))]
struct IntegrationsApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    IntegrationsApi::openapi()
}
