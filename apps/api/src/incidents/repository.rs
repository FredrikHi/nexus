use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::model::{IncidentEventKind, IncidentEventRow, IncidentRow, Severity};

/// What the reconciler needs about one integration: its current health, how
/// much it matters, and whether it already has an open incident.
pub struct ReconcileInput {
    pub integration_id: Uuid,
    pub organization_id: Uuid,
    pub integration_name: String,
    pub health_status: String,
    pub criticality: String,
    pub reason: String,
    pub error_rate: f64,
    pub event_count: i64,
    pub open_incident_id: Option<Uuid>,
    pub open_incident_severity: Option<String>,
}

/// Everything the reconciler needs, in one query.
///
/// The LEFT JOIN onto incidents uses `resolved_at IS NULL`, which is exactly
/// the predicate of the partial unique index, so at most one row can match and
/// the join cannot fan out.
pub async fn gather(db: &PgPool, org_id: Uuid) -> Result<Vec<ReconcileInput>, ApiError> {
    let rows = sqlx::query!(
        r#"SELECT
             h.integration_id  AS "integration_id!",
             h.organization_id AS "organization_id!",
             i.name            AS "integration_name!",
             h.status          AS "health_status!",
             i.criticality     AS "criticality!",
             h.reason          AS "reason!",
             h.error_rate      AS "error_rate!",
             h.event_count     AS "event_count!",
             inc.id            AS "open_incident_id?",
             inc.severity      AS "open_incident_severity?"
           FROM integration_health h
           JOIN integrations i ON i.id = h.integration_id
           LEFT JOIN incidents inc
                  ON inc.integration_id = h.integration_id
                 AND inc.resolved_at IS NULL
           WHERE h.organization_id = $1"#,
        org_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ReconcileInput {
            integration_id: r.integration_id,
            organization_id: r.organization_id,
            integration_name: r.integration_name,
            health_status: r.health_status,
            criticality: r.criticality,
            reason: r.reason,
            error_rate: r.error_rate,
            event_count: r.event_count,
            open_incident_id: r.open_incident_id,
            open_incident_severity: r.open_incident_severity,
        })
        .collect())
}

/// Opens an incident, or returns None if one was already open.
///
/// `ON CONFLICT DO NOTHING` leans on the partial unique index rather than on
/// the reconciler having checked first: two reconcilers racing produce one
/// incident and one no-op, not a duplicate.
pub async fn open(
    db: &PgPool,
    input: &ReconcileInput,
    severity: Severity,
    title: &str,
) -> Result<Option<Uuid>, ApiError> {
    let id = sqlx::query_scalar!(
        r#"INSERT INTO incidents
             (organization_id, integration_id, severity, title, summary,
              opened_error_rate, opened_event_count)
           VALUES ($1,$2,$3,$4,$5,$6,$7)
           ON CONFLICT (integration_id) WHERE resolved_at IS NULL DO NOTHING
           RETURNING id"#,
        input.organization_id,
        input.integration_id,
        severity.as_str(),
        title,
        input.reason,
        input.error_rate,
        input.event_count
    )
    .fetch_optional(db)
    .await?;

    Ok(id)
}

/// Raises severity and refreshes the summary. Severity never falls while the
/// incident is open: what mattered most is the useful number afterwards.
pub async fn escalate(
    db: &PgPool,
    incident_id: Uuid,
    severity: Severity,
    summary: &str,
) -> Result<(), ApiError> {
    sqlx::query!(
        "UPDATE incidents SET severity = $2, summary = $3 WHERE id = $1",
        incident_id,
        severity.as_str(),
        summary
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Keeps the summary current without changing severity.
pub async fn refresh_summary(
    db: &PgPool,
    incident_id: Uuid,
    summary: &str,
) -> Result<(), ApiError> {
    sqlx::query!(
        "UPDATE incidents SET summary = $2 WHERE id = $1 AND summary IS DISTINCT FROM $2",
        incident_id,
        summary
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn resolve(db: &PgPool, incident_id: Uuid, summary: &str) -> Result<(), ApiError> {
    sqlx::query!(
        r#"UPDATE incidents
           SET status = 'RESOLVED', resolved_at = now(), summary = $2
           WHERE id = $1 AND resolved_at IS NULL"#,
        incident_id,
        summary
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Appends to the timeline. `actor` is None when the system acted.
pub async fn add_event(
    db: &PgPool,
    incident_id: Uuid,
    kind: IncidentEventKind,
    message: &str,
    actor: Option<&str>,
) -> Result<(), ApiError> {
    sqlx::query!(
        "INSERT INTO incident_events (incident_id, kind, message, actor) VALUES ($1,$2,$3,$4)",
        incident_id,
        kind.as_str(),
        message,
        actor
    )
    .execute(db)
    .await?;

    Ok(())
}

/// Lists incidents, newest first, with every filter optional.
///
/// `only_open` maps to `resolved_at IS NULL`, matching the partial index, so
/// the common dashboard query uses it directly.
pub async fn list(
    db: &PgPool,
    org_id: Uuid,
    integration_id: Option<Uuid>,
    severity: Option<&str>,
    status: Option<&str>,
    only_open: bool,
    limit: i64,
) -> Result<Vec<IncidentRow>, ApiError> {
    let rows = sqlx::query_as!(
        IncidentRow,
        r#"SELECT id, organization_id, integration_id, status, severity, title, summary,
                  opened_at, acknowledged_at, acknowledged_by, resolved_at,
                  opened_error_rate, opened_event_count
           FROM incidents
           WHERE organization_id = $1
             AND ($2::uuid IS NULL OR integration_id = $2)
             AND ($3::text IS NULL OR severity = $3)
             AND ($4::text IS NULL OR status = $4)
             AND ($5::bool IS NOT TRUE OR resolved_at IS NULL)
           ORDER BY resolved_at IS NOT NULL, opened_at DESC
           LIMIT $6"#,
        org_id,
        integration_id,
        severity,
        status,
        only_open,
        limit
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

pub async fn find(db: &PgPool, org_id: Uuid, id: Uuid) -> Result<Option<IncidentRow>, ApiError> {
    let row = sqlx::query_as!(
        IncidentRow,
        r#"SELECT id, organization_id, integration_id, status, severity, title, summary,
                  opened_at, acknowledged_at, acknowledged_by, resolved_at,
                  opened_error_rate, opened_event_count
           FROM incidents
           WHERE organization_id = $1 AND id = $2"#,
        org_id,
        id
    )
    .fetch_optional(db)
    .await?;

    Ok(row)
}

pub async fn timeline(db: &PgPool, incident_id: Uuid) -> Result<Vec<IncidentEventRow>, ApiError> {
    let rows = sqlx::query_as!(
        IncidentEventRow,
        r#"SELECT id, kind, message, actor, created_at
           FROM incident_events
           WHERE incident_id = $1
           ORDER BY created_at ASC, kind ASC"#,
        incident_id
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

/// Acknowledges an open incident. Returns false if it was already
/// acknowledged or has since resolved, so the caller can answer honestly.
pub async fn acknowledge(
    db: &PgPool,
    org_id: Uuid,
    id: Uuid,
    actor: &str,
) -> Result<bool, ApiError> {
    let result = sqlx::query!(
        r#"UPDATE incidents
           SET status = 'ACKNOWLEDGED', acknowledged_at = now(), acknowledged_by = $3
           WHERE id = $1 AND organization_id = $2
             AND resolved_at IS NULL AND acknowledged_at IS NULL"#,
        id,
        org_id,
        actor
    )
    .execute(db)
    .await?;

    Ok(result.rows_affected() > 0)
}
