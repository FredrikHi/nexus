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
        .route("/api/v1/teams", get(handlers::list).post(handlers::create))
        .route(
            "/api/v1/teams/{id}",
            get(handlers::get).patch(handlers::update).delete(handlers::delete),
        )
        .route(
            "/api/v1/teams/{id}/members",
            get(handlers::members).post(handlers::add_member),
        )
        .route(
            "/api/v1/teams/{id}/members/{user_id}",
            delete(handlers::remove_member),
        )
}

#[derive(OpenApi)]
#[openapi(paths(
    handlers::list, handlers::create, handlers::get, handlers::update,
    handlers::delete, handlers::members, handlers::add_member, handlers::remove_member,
))]
struct TeamsApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    TeamsApi::openapi()
}
