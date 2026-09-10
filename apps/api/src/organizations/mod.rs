mod dto;
mod handlers;
mod model;
mod repository;
mod service;

use axum::routing::{get, patch, post};
use axum::Router;
use utoipa::OpenApi;

use crate::AppState;

// The health worker sweeps every tenant, so it needs the list of them.
pub use repository::all_ids;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/me", get(handlers::me))
        .route("/api/v1/organizations", post(handlers::create))
        .route("/api/v1/organization", get(handlers::current))
        .route(
            "/api/v1/organization/members",
            get(handlers::members).post(handlers::add_member),
        )
        .route(
            "/api/v1/organization/members/{user_id}",
            patch(handlers::set_role).delete(handlers::remove_member),
        )
}

#[derive(OpenApi)]
#[openapi(paths(
    handlers::me,
    handlers::create,
    handlers::current,
    handlers::members,
    handlers::add_member,
    handlers::set_role,
    handlers::remove_member,
))]
struct OrganizationsApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    OrganizationsApi::openapi()
}
