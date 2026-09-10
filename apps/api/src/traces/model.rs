use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::ApiError;
use crate::telemetry::{TelemetryEvent, TelemetryStatus};

/// One correlated flow: every telemetry event sharing a trace_id.
///
/// Derived from telemetry_events rather than stored. A trace has no identity of
/// its own beyond the id its spans carry, so materialising one would introduce
/// a second source of truth that could disagree with the events.
#[derive(Debug, Serialize, ToSchema)]
pub struct TraceSummary {
    pub trace_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    /// Wall time from the first span to the last, in milliseconds.
    pub elapsed_ms: f64,
    pub span_count: i64,
    /// How many distinct integrations the flow crossed.
    pub integration_count: i64,
    /// Spans that were FAILURE or TIMEOUT. REJECTED is excluded for the same
    /// reason it is excluded from the telemetry error rate.
    pub error_count: i64,
    /// Worst status in the trace, by the precedence
    /// FAILURE > TIMEOUT > REJECTED > SUCCESS. A flow is only as healthy as
    /// its unhealthiest hop.
    pub status: TelemetryStatus,
}

/// A trace and the spans that make it up, oldest first.
#[derive(Debug, Serialize, ToSchema)]
pub struct TraceDetail {
    #[serde(flatten)]
    pub summary: TraceSummary,
    pub spans: Vec<TelemetryEvent>,
}

/// The raw aggregate row; the rolled-up status arrives as `String`.
#[derive(Debug)]
pub struct TraceSummaryRow {
    pub trace_id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub elapsed_ms: f64,
    pub span_count: i64,
    pub integration_count: i64,
    pub error_count: i64,
    pub status: String,
}

impl TraceSummaryRow {
    pub fn into_domain(self) -> Result<TraceSummary, ApiError> {
        let status = TelemetryStatus::from_db(&self.status)
            .ok_or_else(|| ApiError::Internal(format!("unknown status '{}'", self.status)))?;

        Ok(TraceSummary {
            trace_id: self.trace_id,
            started_at: self.started_at,
            ended_at: self.ended_at,
            elapsed_ms: self.elapsed_ms,
            span_count: self.span_count,
            integration_count: self.integration_count,
            error_count: self.error_count,
            status,
        })
    }
}
