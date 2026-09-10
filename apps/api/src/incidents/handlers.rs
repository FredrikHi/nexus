use axum::extract::State;
use uuid::Uuid;

use crate::error::{ApiError, ErrorBody};
use crate::extract::{Json, Path, Query};
use crate::AppState;

use super::dto::{AcknowledgeIncident, AddNote, ListIncidentsQuery};
use super::model::{Incident, IncidentDetail, ReconcileResult};
use super::service;

#[utoipa::path(
    get,
    path = "/api/v1/incidents",
    tag = "Incidents",
    params(ListIncidentsQuery),
    responses(
        (status = 200, description = "Incidents, unresolved first then newest first",
         body = Vec<Incident>),
        (status = 400, description = "Malformed query parameter", body = ErrorBody),
    )
)]
pub async fn list(
    State(state): State<AppState>,
    Query(params): Query<ListIncidentsQuery>,
) -> Result<Json<Vec<Incident>>, ApiError> {
    Ok(Json(service::list(&state.db, params).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/incidents/{id}",
    tag = "Incidents",
    params(("id" = Uuid, Path, description = "Incident id")),
    responses(
        (status = 200, description = "The incident and its timeline, oldest first",
         body = IncidentDetail),
        (status = 404, description = "No such incident", body = ErrorBody),
    )
)]
pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<IncidentDetail>, ApiError> {
    Ok(Json(service::get(&state.db, id).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/incidents/{id}/acknowledge",
    tag = "Incidents",
    params(("id" = Uuid, Path, description = "Incident id")),
    request_body = AcknowledgeIncident,
    responses(
        (status = 200, description = "Acknowledged. Still open: only recovery resolves it.",
         body = IncidentDetail),
        (status = 404, description = "No such incident", body = ErrorBody),
        (status = 409, description = "Already acknowledged, or already resolved", body = ErrorBody),
    )
)]
pub async fn acknowledge(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AcknowledgeIncident>,
) -> Result<Json<IncidentDetail>, ApiError> {
    Ok(Json(service::acknowledge(&state.db, id, body).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/incidents/{id}/notes",
    tag = "Incidents",
    params(("id" = Uuid, Path, description = "Incident id")),
    request_body = AddNote,
    responses(
        (status = 200, description = "Note appended to the timeline", body = IncidentDetail),
        (status = 404, description = "No such incident", body = ErrorBody),
        (status = 422, description = "Empty actor or message", body = ErrorBody),
    )
)]
pub async fn add_note(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<AddNote>,
) -> Result<Json<IncidentDetail>, ApiError> {
    Ok(Json(service::add_note(&state.db, id, body).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/incidents/reconcile",
    tag = "Incidents",
    responses(
        (status = 200, description = "Reconciliation ran; reports what changed",
         body = ReconcileResult),
    )
)]
pub async fn reconcile(
    State(state): State<AppState>,
) -> Result<Json<ReconcileResult>, ApiError> {
    // The same pass the worker runs after each evaluation. Idempotent, so
    // calling it by hand is safe and useful for a dashboard refresh.
    Ok(Json(service::reconcile_default_org(&state.db).await?))
}
