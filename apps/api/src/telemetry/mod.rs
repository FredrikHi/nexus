mod dto;
mod handlers;
mod model;
mod repository;
mod service;

#[cfg(test)]
mod tests;

use axum::routing::get;
use axum::Router;
use sqlx::PgPool;
use utoipa::OpenApi;

use crate::error::ApiError;

// A span in a trace is a telemetry event, so the traces feature reads these
// types rather than defining near-duplicates of them.
use crate::AppState;
pub use model::{TelemetryEvent, TelemetryEventRow, TelemetryStatus};

/// Creates the monthly partitions around now. Called once at startup so ingest
/// never hits a month with nowhere to put a row. Idempotent.
pub async fn ensure_partitions(db: &PgPool) -> Result<(), ApiError> {
    repository::ensure_partitions(db, 2).await
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/telemetry",
            get(handlers::list).post(handlers::ingest),
        )
        .route("/api/v1/telemetry/summary", get(handlers::summary))
        .route("/api/v1/telemetry/series", get(handlers::series))
}

#[derive(OpenApi)]
#[openapi(paths(handlers::ingest, handlers::list, handlers::summary, handlers::series))]
struct TelemetryApi;

pub fn openapi() -> utoipa::openapi::OpenApi {
    TelemetryApi::openapi()
}
