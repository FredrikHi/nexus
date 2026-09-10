use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;

/// The JSON body we return from the health endpoints.
/// `#[derive(Serialize)]` teaches serde how to turn this struct into JSON.
#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
}

/// Liveness: "is the process up and answering HTTP?"
/// No database involved on purpose.
#[utoipa::path(
    get,
    path = "/api/v1/health",
    tag = "Service health",
    responses((status = 200, description = "The process is up", body = HealthResponse))
)]
pub async fn liveness() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// Readiness: "can I actually reach my dependencies (the DB)?"
#[utoipa::path(
    get,
    path = "/api/v1/health/ready",
    tag = "Service health",
    responses(
        (status = 200, description = "The database is reachable", body = HealthResponse),
        (status = 503, description = "The database is not reachable"),
    )
)]
pub async fn readiness(State(state): State<AppState>) -> Result<Json<HealthResponse>, StatusCode> {
    match sqlx::query("SELECT 1").execute(&state.db).await {
        Ok(_) => Ok(Json(HealthResponse { status: "ok" })),
        Err(err) => {
            tracing::error!(error = %err, "database readiness check failed");
            Err(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}
