use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::Criticality;
use crate::error::ApiError;
use crate::integration_health::HealthStatus;

/// How much this incident matters.
///
/// Combines how broken the integration is with how much it matters to the
/// business. A degraded payments feed outranks a completely dead internal
/// report, and only one of those should wake anyone up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Severity {
    Minor,
    Major,
    Critical,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Minor => "MINOR",
            Severity::Major => "MAJOR",
            Severity::Critical => "CRITICAL",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "MINOR" => Some(Severity::Minor),
            "MAJOR" => Some(Severity::Major),
            "CRITICAL" => Some(Severity::Critical),
            _ => None,
        }
    }

    /// Derives severity from how broken something is and how much it matters.
    ///
    /// Only DEGRADED and UNHEALTHY reach here; a healthy or unknown
    /// integration has no incident to rate, which is why this returns Option.
    pub fn derive(health: HealthStatus, criticality: Criticality) -> Option<Self> {
        let important = matches!(criticality, Criticality::High | Criticality::Critical);

        match (health, important) {
            (HealthStatus::Unhealthy, true) => Some(Severity::Critical),
            (HealthStatus::Unhealthy, false) => Some(Severity::Major),
            (HealthStatus::Degraded, true) => Some(Severity::Major),
            (HealthStatus::Degraded, false) => Some(Severity::Minor),
            _ => None,
        }
    }
}

/// Where an incident stands. ACKNOWLEDGED still means open: someone is looking
/// at it, nothing is fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IncidentStatus {
    Open,
    Acknowledged,
    Resolved,
}

impl IncidentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            IncidentStatus::Open => "OPEN",
            IncidentStatus::Acknowledged => "ACKNOWLEDGED",
            IncidentStatus::Resolved => "RESOLVED",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "OPEN" => Some(IncidentStatus::Open),
            "ACKNOWLEDGED" => Some(IncidentStatus::Acknowledged),
            "RESOLVED" => Some(IncidentStatus::Resolved),
            _ => None,
        }
    }
}

/// What happened to an incident, and whether a person or the system did it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IncidentEventKind {
    Opened,
    Escalated,
    Acknowledged,
    Note,
    Resolved,
}

impl IncidentEventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            IncidentEventKind::Opened => "OPENED",
            IncidentEventKind::Escalated => "ESCALATED",
            IncidentEventKind::Acknowledged => "ACKNOWLEDGED",
            IncidentEventKind::Note => "NOTE",
            IncidentEventKind::Resolved => "RESOLVED",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "OPENED" => Some(IncidentEventKind::Opened),
            "ESCALATED" => Some(IncidentEventKind::Escalated),
            "ACKNOWLEDGED" => Some(IncidentEventKind::Acknowledged),
            "NOTE" => Some(IncidentEventKind::Note),
            "RESOLVED" => Some(IncidentEventKind::Resolved),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Incident {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub integration_id: Uuid,
    pub status: IncidentStatus,
    /// Peak severity, never lowered while the incident is open.
    pub severity: Severity,
    pub title: String,
    pub summary: String,
    pub opened_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    /// Health when it opened, kept so the incident still means something once
    /// the telemetry behind it ages out of retention.
    pub opened_error_rate: Option<f64>,
    pub opened_event_count: Option<i64>,
}

#[derive(Debug)]
pub struct IncidentRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub integration_id: Uuid,
    pub status: String,
    pub severity: String,
    pub title: String,
    pub summary: String,
    pub opened_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub opened_error_rate: Option<f64>,
    pub opened_event_count: Option<i64>,
}

impl IncidentRow {
    pub fn into_domain(self) -> Result<Incident, ApiError> {
        let status = IncidentStatus::from_db(&self.status).ok_or_else(|| {
            ApiError::Internal(format!("unknown incident status '{}'", self.status))
        })?;
        let severity = Severity::from_db(&self.severity)
            .ok_or_else(|| ApiError::Internal(format!("unknown severity '{}'", self.severity)))?;

        Ok(Incident {
            id: self.id,
            organization_id: self.organization_id,
            integration_id: self.integration_id,
            status,
            severity,
            title: self.title,
            summary: self.summary,
            opened_at: self.opened_at,
            acknowledged_at: self.acknowledged_at,
            acknowledged_by: self.acknowledged_by,
            resolved_at: self.resolved_at,
            opened_error_rate: self.opened_error_rate,
            opened_event_count: self.opened_event_count,
        })
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IncidentEvent {
    pub id: Uuid,
    pub kind: IncidentEventKind,
    pub message: String,
    /// None when the system did it rather than a person.
    pub actor: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct IncidentEventRow {
    pub id: Uuid,
    pub kind: String,
    pub message: String,
    pub actor: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl IncidentEventRow {
    pub fn into_domain(self) -> Result<IncidentEvent, ApiError> {
        let kind = IncidentEventKind::from_db(&self.kind)
            .ok_or_else(|| ApiError::Internal(format!("unknown event kind '{}'", self.kind)))?;

        Ok(IncidentEvent {
            id: self.id,
            kind,
            message: self.message,
            actor: self.actor,
            created_at: self.created_at,
        })
    }
}

/// An incident with everything that has happened to it, oldest first.
#[derive(Debug, Serialize, ToSchema)]
pub struct IncidentDetail {
    #[serde(flatten)]
    pub incident: Incident,
    pub timeline: Vec<IncidentEvent>,
}

/// What one reconciliation pass did.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReconcileResult {
    pub opened: usize,
    pub escalated: usize,
    pub resolved: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn business_criticality_lifts_severity() {
        // The same brokenness matters differently depending on what broke.
        assert_eq!(
            Severity::derive(HealthStatus::Degraded, Criticality::Low),
            Some(Severity::Minor)
        );
        assert_eq!(
            Severity::derive(HealthStatus::Degraded, Criticality::Critical),
            Some(Severity::Major)
        );
        assert_eq!(
            Severity::derive(HealthStatus::Unhealthy, Criticality::Low),
            Some(Severity::Major)
        );
        assert_eq!(
            Severity::derive(HealthStatus::Unhealthy, Criticality::High),
            Some(Severity::Critical)
        );
    }

    #[test]
    fn a_dead_critical_feed_outranks_a_dead_trivial_one() {
        let payments = Severity::derive(HealthStatus::Unhealthy, Criticality::Critical).unwrap();
        let internal = Severity::derive(HealthStatus::Unhealthy, Criticality::Low).unwrap();
        assert!(payments > internal);
    }

    #[test]
    fn healthy_and_unknown_produce_no_incident() {
        assert_eq!(Severity::derive(HealthStatus::Healthy, Criticality::Critical), None);
        assert_eq!(Severity::derive(HealthStatus::Unknown, Criticality::Critical), None);
    }

    #[test]
    fn severity_orders_from_minor_to_critical() {
        // Ord is derived, and escalation compares with it.
        assert!(Severity::Minor < Severity::Major);
        assert!(Severity::Major < Severity::Critical);
    }
}
