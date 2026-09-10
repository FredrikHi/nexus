use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::ApiError;

/// The verdict for one integration over its evaluation window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    /// Not enough traffic to judge. Deliberately distinct from UNHEALTHY:
    /// silence is not failure, and plenty of integrations are idle by design
    /// between scheduled runs.
    Unknown,
}

impl HealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            HealthStatus::Healthy => "HEALTHY",
            HealthStatus::Degraded => "DEGRADED",
            HealthStatus::Unhealthy => "UNHEALTHY",
            HealthStatus::Unknown => "UNKNOWN",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "HEALTHY" => Some(HealthStatus::Healthy),
            "DEGRADED" => Some(HealthStatus::Degraded),
            "UNHEALTHY" => Some(HealthStatus::Unhealthy),
            "UNKNOWN" => Some(HealthStatus::Unknown),
            _ => None,
        }
    }
}

/// Thresholds resolved for one integration: its own policy if it has one,
/// otherwise the organization default.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedPolicy {
    pub window_minutes: i32,
    pub min_events: i32,
    pub error_rate_degraded: f64,
    pub error_rate_unhealthy: f64,
    pub p95_degraded_ms: Option<i32>,
    pub p95_unhealthy_ms: Option<i32>,
    pub stale_after_minutes: i32,
}

/// What the evaluator measured for one integration, before judging it.
#[derive(Debug, Clone, Copy)]
pub struct Measurement {
    pub event_count: i64,
    pub error_count: i64,
    pub p95_duration_ms: Option<f64>,
    pub last_event_at: Option<DateTime<Utc>>,
}

impl Measurement {
    pub fn error_rate(&self) -> f64 {
        if self.event_count == 0 {
            0.0
        } else {
            self.error_count as f64 / self.event_count as f64
        }
    }
}

/// A verdict plus the sentence explaining it.
#[derive(Debug, Clone)]
pub struct Verdict {
    pub status: HealthStatus,
    pub reason: String,
}

/// Current health of one integration, as the API returns it.
#[derive(Debug, Serialize, ToSchema)]
pub struct IntegrationHealth {
    pub integration_id: Uuid,
    pub organization_id: Uuid,
    pub status: HealthStatus,
    /// When the CURRENT status began, not when it was last evaluated.
    pub since: DateTime<Utc>,
    pub evaluated_at: DateTime<Utc>,
    pub window_minutes: i32,
    pub event_count: i64,
    pub error_count: i64,
    pub error_rate: f64,
    pub p95_duration_ms: Option<f64>,
    pub last_event_at: Option<DateTime<Utc>>,
    /// Why the evaluator decided this, in words.
    pub reason: String,
}

#[derive(Debug)]
pub struct IntegrationHealthRow {
    pub integration_id: Uuid,
    pub organization_id: Uuid,
    pub status: String,
    pub since: DateTime<Utc>,
    pub evaluated_at: DateTime<Utc>,
    pub window_minutes: i32,
    pub event_count: i64,
    pub error_count: i64,
    pub error_rate: f64,
    pub p95_duration_ms: Option<f64>,
    pub last_event_at: Option<DateTime<Utc>>,
    pub reason: String,
}

impl IntegrationHealthRow {
    pub fn into_domain(self) -> Result<IntegrationHealth, ApiError> {
        let status = HealthStatus::from_db(&self.status)
            .ok_or_else(|| ApiError::Internal(format!("unknown health status '{}'", self.status)))?;

        Ok(IntegrationHealth {
            integration_id: self.integration_id,
            organization_id: self.organization_id,
            status,
            since: self.since,
            evaluated_at: self.evaluated_at,
            window_minutes: self.window_minutes,
            event_count: self.event_count,
            error_count: self.error_count,
            error_rate: self.error_rate,
            p95_duration_ms: self.p95_duration_ms,
            last_event_at: self.last_event_at,
            reason: self.reason,
        })
    }
}

/// One recorded status change.
#[derive(Debug, Serialize, ToSchema)]
pub struct HealthTransition {
    pub id: Uuid,
    pub integration_id: Uuid,
    /// None on the very first evaluation of an integration.
    pub from_status: Option<HealthStatus>,
    pub to_status: HealthStatus,
    pub changed_at: DateTime<Utc>,
    pub reason: String,
    pub error_rate: Option<f64>,
    pub event_count: Option<i64>,
}

#[derive(Debug)]
pub struct HealthTransitionRow {
    pub id: Uuid,
    pub integration_id: Uuid,
    pub from_status: Option<String>,
    pub to_status: String,
    pub changed_at: DateTime<Utc>,
    pub reason: String,
    pub error_rate: Option<f64>,
    pub event_count: Option<i64>,
}

impl HealthTransitionRow {
    pub fn into_domain(self) -> Result<HealthTransition, ApiError> {
        let parse = |s: &str| {
            HealthStatus::from_db(s)
                .ok_or_else(|| ApiError::Internal(format!("unknown health status '{s}'")))
        };

        Ok(HealthTransition {
            id: self.id,
            integration_id: self.integration_id,
            from_status: self.from_status.as_deref().map(parse).transpose()?,
            to_status: parse(&self.to_status)?,
            changed_at: self.changed_at,
            reason: self.reason,
            error_rate: self.error_rate,
            event_count: self.event_count,
        })
    }
}

/// What one evaluation pass did.
#[derive(Debug, Serialize, ToSchema)]
pub struct EvaluationResult {
    pub evaluated: usize,
    pub changed: usize,
}
