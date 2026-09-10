use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::ApiError;

/// Mirrors the `status` TEXT + CHECK column on telemetry_events.
///
/// FAILURE and REJECTED are separate on purpose. Only one of them says the
/// integration is unhealthy; the other says the caller sent something wrong.
/// Collapsing them would make every validation error look like an outage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TelemetryStatus {
    Success,
    Failure,
    Timeout,
    Rejected,
}

impl TelemetryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TelemetryStatus::Success => "SUCCESS",
            TelemetryStatus::Failure => "FAILURE",
            TelemetryStatus::Timeout => "TIMEOUT",
            TelemetryStatus::Rejected => "REJECTED",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "SUCCESS" => Some(TelemetryStatus::Success),
            "FAILURE" => Some(TelemetryStatus::Failure),
            "TIMEOUT" => Some(TelemetryStatus::Timeout),
            "REJECTED" => Some(TelemetryStatus::Rejected),
            _ => None,
        }
    }
}

/// One recorded call across one integration.
#[derive(Debug, Serialize, ToSchema)]
pub struct TelemetryEvent {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub integration_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub occurred_at: DateTime<Utc>,
    pub status: TelemetryStatus,
    pub duration_ms: Option<i32>,
    pub trace_id: Option<String>,
    pub operation: Option<String>,
    pub status_code: Option<i32>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub payload_bytes: Option<i64>,
    pub metadata: serde_json::Value,
    /// When the platform accepted it, as opposed to when it happened.
    pub received_at: DateTime<Utc>,
}

/// The raw DB row; the enum column arrives as `String`.
#[derive(Debug)]
pub struct TelemetryEventRow {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub integration_id: Uuid,
    pub environment_id: Option<Uuid>,
    pub occurred_at: DateTime<Utc>,
    pub status: String,
    pub duration_ms: Option<i32>,
    pub trace_id: Option<String>,
    pub operation: Option<String>,
    pub status_code: Option<i32>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub payload_bytes: Option<i64>,
    pub metadata: serde_json::Value,
    pub received_at: DateTime<Utc>,
}

impl TelemetryEventRow {
    pub fn into_domain(self) -> Result<TelemetryEvent, ApiError> {
        let status = TelemetryStatus::from_db(&self.status)
            .ok_or_else(|| ApiError::Internal(format!("unknown status '{}'", self.status)))?;

        Ok(TelemetryEvent {
            id: self.id,
            organization_id: self.organization_id,
            integration_id: self.integration_id,
            environment_id: self.environment_id,
            occurred_at: self.occurred_at,
            status,
            duration_ms: self.duration_ms,
            trace_id: self.trace_id,
            operation: self.operation,
            status_code: self.status_code,
            error_type: self.error_type,
            error_message: self.error_message,
            payload_bytes: self.payload_bytes,
            metadata: self.metadata,
            received_at: self.received_at,
        })
    }
}

/// What the ingest endpoint reports back.
#[derive(Debug, Serialize, ToSchema)]
pub struct IngestResult {
    pub accepted: usize,
}

/// Aggregate health of an integration over a window.
///
/// `error_rate` counts FAILURE and TIMEOUT but not REJECTED, for the reason
/// given on `TelemetryStatus`: a rejected call is the caller's fault, and
/// folding it in here would make a noisy client look like a broken dependency.
#[derive(Debug, Serialize, ToSchema)]
pub struct TelemetrySummary {
    pub total: i64,
    pub success: i64,
    pub failure: i64,
    pub timeout: i64,
    pub rejected: i64,
    pub error_rate: f64,
    /// Percentiles over duration_ms, null when nothing in the window recorded one.
    pub p50_duration_ms: Option<f64>,
    pub p95_duration_ms: Option<f64>,
    pub p99_duration_ms: Option<f64>,
}

/// Raw aggregate row before the derived error rate is computed.
#[derive(Debug)]
pub struct SummaryRow {
    pub total: i64,
    pub success: i64,
    pub failure: i64,
    pub timeout: i64,
    pub rejected: i64,
    pub p50_duration_ms: Option<f64>,
    pub p95_duration_ms: Option<f64>,
    pub p99_duration_ms: Option<f64>,
}

impl SummaryRow {
    pub fn into_domain(self) -> TelemetrySummary {
        let unhealthy = self.failure + self.timeout;
        let error_rate = if self.total == 0 {
            0.0
        } else {
            unhealthy as f64 / self.total as f64
        };

        TelemetrySummary {
            total: self.total,
            success: self.success,
            failure: self.failure,
            timeout: self.timeout,
            rejected: self.rejected,
            error_rate,
            p50_duration_ms: self.p50_duration_ms,
            p95_duration_ms: self.p95_duration_ms,
            p99_duration_ms: self.p99_duration_ms,
        }
    }
}

/// One time bucket of telemetry, for charting.
#[derive(Debug, Serialize, ToSchema)]
pub struct SeriesPoint {
    /// Start of the bucket.
    pub bucket: DateTime<Utc>,
    pub total: i64,
    pub success: i64,
    pub failure: i64,
    pub timeout: i64,
    pub rejected: i64,
    /// FAILURE and TIMEOUT over total, matching the summary endpoint.
    pub error_rate: f64,
    pub p95_duration_ms: Option<f64>,
}

/// The raw bucket row, before the derived rate.
#[derive(Debug)]
pub struct SeriesRow {
    pub bucket: DateTime<Utc>,
    pub total: i64,
    pub success: i64,
    pub failure: i64,
    pub timeout: i64,
    pub rejected: i64,
    pub p95_duration_ms: Option<f64>,
}

impl SeriesRow {
    pub fn into_domain(self) -> SeriesPoint {
        let unhealthy = self.failure + self.timeout;
        let error_rate = if self.total == 0 {
            0.0
        } else {
            unhealthy as f64 / self.total as f64
        };

        SeriesPoint {
            bucket: self.bucket,
            total: self.total,
            success: self.success,
            failure: self.failure,
            timeout: self.timeout,
            rejected: self.rejected,
            error_rate,
            p95_duration_ms: self.p95_duration_ms,
        }
    }
}
