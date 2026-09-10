use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::Criticality;
use crate::error::ApiError;
use crate::integration_health::HealthStatus;
use crate::util::default_org_id;

use super::dto::{AcknowledgeIncident, AddNote, ListIncidentsQuery};
use super::model::{
    Incident, IncidentDetail, IncidentEventKind, ReconcileResult, Severity,
};
use super::repository;

const DEFAULT_LIMIT: i64 = 50;
const MAX_LIMIT: i64 = 500;

/// Brings incidents into line with current health.
///
/// Runs after every evaluation pass and is idempotent: it compares what health
/// says now against what incidents exist, and makes the smallest change that
/// reconciles them. Nothing here depends on catching a transition as it
/// happens, so a missed pass costs a delay rather than a lost incident.
pub async fn reconcile(db: &PgPool, org_id: Uuid) -> Result<ReconcileResult, ApiError> {
    let inputs = repository::gather(db, org_id).await?;
    let mut result = ReconcileResult { opened: 0, escalated: 0, resolved: 0 };

    for input in &inputs {
        let health = HealthStatus::from_db(&input.health_status).ok_or_else(|| {
            ApiError::Internal(format!("unknown health status '{}'", input.health_status))
        })?;
        let criticality = Criticality::from_db(&input.criticality).ok_or_else(|| {
            ApiError::Internal(format!("unknown criticality '{}'", input.criticality))
        })?;

        let warranted = Severity::derive(health, criticality);

        match (warranted, input.open_incident_id) {
            // Broken, and nothing open yet: open one.
            (Some(severity), None) => {
                let title = format!("{} is {}", input.integration_name, health.as_str().to_lowercase());
                if let Some(id) = repository::open(db, input, severity, &title).await? {
                    repository::add_event(
                        db,
                        id,
                        IncidentEventKind::Opened,
                        &format!("Opened at {} severity. {}", severity.as_str(), input.reason),
                        None,
                    )
                    .await?;
                    result.opened += 1;
                }
            }

            // Broken, and already tracked: escalate if it got worse.
            (Some(severity), Some(id)) => {
                let current = input
                    .open_incident_severity
                    .as_deref()
                    .and_then(Severity::from_db)
                    .unwrap_or(Severity::Minor);

                if severity > current {
                    repository::escalate(db, id, severity, &input.reason).await?;
                    repository::add_event(
                        db,
                        id,
                        IncidentEventKind::Escalated,
                        &format!(
                            "Escalated from {} to {}. {}",
                            current.as_str(),
                            severity.as_str(),
                            input.reason
                        ),
                        None,
                    )
                    .await?;
                    result.escalated += 1;
                } else {
                    // Severity never falls while open, but keep the summary
                    // current so the incident reflects what is happening now.
                    repository::refresh_summary(db, id, &input.reason).await?;
                }
            }

            // Recovered: resolve. Only HEALTHY counts, which is why this arm
            // tests the status rather than just the absence of a severity.
            (None, Some(id)) if health == HealthStatus::Healthy => {
                repository::resolve(db, id, &input.reason).await?;
                repository::add_event(
                    db,
                    id,
                    IncidentEventKind::Resolved,
                    &format!("Recovered. {}", input.reason),
                    None,
                )
                .await?;
                result.resolved += 1;
            }

            // UNKNOWN with an incident open: leave it open. An integration
            // that stopped reporting while broken has not been fixed.
            (None, Some(_)) => {}

            // Healthy or unknown, nothing open: nothing to do.
            (None, None) => {}
        }
    }

    Ok(result)
}

pub async fn reconcile_default_org(db: &PgPool) -> Result<ReconcileResult, ApiError> {
    reconcile(db, default_org_id()).await
}

pub async fn list(db: &PgPool, query: ListIncidentsQuery) -> Result<Vec<Incident>, ApiError> {
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);

    repository::list(
        db,
        default_org_id(),
        query.integration_id,
        query.severity.map(|s| s.as_str()),
        query.status.map(|s| s.as_str()),
        query.only_open.unwrap_or(false),
        limit,
    )
    .await?
    .into_iter()
    .map(|r| r.into_domain())
    .collect()
}

pub async fn get(db: &PgPool, id: Uuid) -> Result<IncidentDetail, ApiError> {
    let incident = repository::find(db, default_org_id(), id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("incident {id} not found")))?
        .into_domain()?;

    let timeline = repository::timeline(db, id)
        .await?
        .into_iter()
        .map(|r| r.into_domain())
        .collect::<Result<Vec<_>, _>>()?;

    Ok(IncidentDetail { incident, timeline })
}

/// Marks an incident as being looked at. Does not resolve it: only recovery
/// does that, and pretending otherwise would hide a live problem.
pub async fn acknowledge(
    db: &PgPool,
    id: Uuid,
    input: AcknowledgeIncident,
) -> Result<IncidentDetail, ApiError> {
    let actor = input.actor.trim();
    if actor.is_empty() {
        return Err(ApiError::Validation("actor must not be empty".to_string()));
    }

    // Confirm it exists before reporting on its state, so a bad id is a clean
    // 404 rather than a confusing conflict.
    let existing = get(db, id).await?;

    if repository::acknowledge(db, default_org_id(), id, actor).await? {
        repository::add_event(
            db,
            id,
            IncidentEventKind::Acknowledged,
            &format!("Acknowledged by {actor}."),
            Some(actor),
        )
        .await?;
        return get(db, id).await;
    }

    Err(ApiError::Conflict(format!(
        "incident {id} is {} and cannot be acknowledged",
        existing.incident.status.as_str()
    )))
}

pub async fn add_note(db: &PgPool, id: Uuid, input: AddNote) -> Result<IncidentDetail, ApiError> {
    let actor = input.actor.trim();
    let message = input.message.trim();
    if actor.is_empty() {
        return Err(ApiError::Validation("actor must not be empty".to_string()));
    }
    if message.is_empty() {
        return Err(ApiError::Validation("message must not be empty".to_string()));
    }

    // 404 before writing, and notes stay allowed after resolution so a
    // postmortem can be written where the incident lives.
    get(db, id).await?;

    repository::add_event(db, id, IncidentEventKind::Note, message, Some(actor)).await?;
    get(db, id).await
}
