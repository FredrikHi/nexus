mod dto;
mod handlers;
mod model;
mod repository;
mod service;

#[cfg(test)]
mod tests;

use axum::routing::{get, patch};
use axum::Router;
use utoipa::OpenApi;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/environments", get(handlers::list).post(handlers::create))
        .route(
            "/api/v1/environments/{id}",
            patch(handlers::update).delete(handlers::delete),
        )
}

#[derive(OpenApi)]
#[openapi(paths(handlers::list, handlers::create, handlers::update, handlers::delete))]
struct EnvironmentsApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    EnvironmentsApi::openapi()
}
