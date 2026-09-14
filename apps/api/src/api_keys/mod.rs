mod auth;
mod dto;
mod handlers;
mod model;
mod repository;
mod service;
mod token;

use axum::routing::{delete, get};
use axum::Router;
use utoipa::OpenApi;

use crate::AppState;

// Two things leave this module beyond its routes: the extractor other features
// authenticate with, and the identity that extractor produces.
pub use auth::ApiKeyAuth;
pub use model::AuthenticatedKey;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/api-keys",
            get(handlers::list).post(handlers::create),
        )
        .route("/api/v1/api-keys/{id}", delete(handlers::revoke))
}

#[derive(OpenApi)]
#[openapi(paths(handlers::list, handlers::create, handlers::revoke))]
struct ApiKeysApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    ApiKeysApi::openapi()
}
