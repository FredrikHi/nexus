use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::model::TelemetryStatus;

/// One event inside an ingest batch.
///
/// The integration is named either by `integration` (its slug) or by
/// `integration_id`. Exactly one is required.
///
/// Prefer the slug. It is stable across instances, so the same build reports
/// to a laptop and to production without a generated file of ids that has to
/// be kept in step with one particular database.
#[derive(Debug, Deserialize, ToSchema)]
pub struct IngestEvent {
    /// Slug of the integration, e.g. "bruno-to-groq".
    pub integration: Option<String>,
    /// Id of the integration. Accepted for existing clients.
    pub integration_id: Option<Uuid>,
    /// Defaults to now if the client does not timestamp its own event.
    pub occurred_at: Option<DateTime<Utc>>,
    pub status: TelemetryStatus,
    /// Omit rather than guessing; percentiles ignore missing values.
    pub duration_ms: Option<i32>,
    /// Correlation handle. Events sharing one become a trace.
    pub trace_id: Option<String>,
    /// What was called, e.g. "POST /invoices".
    pub operation: Option<String>,
    pub status_code: Option<i32>,
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    pub payload_bytes: Option<i64>,
    /// Ignored when the API key is pinned to an environment and disagrees.
    pub environment_id: Option<Uuid>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

/// Request body for POST /telemetry.
///
/// Always a batch, even for one event. A single-event endpoint would invite a
/// request per call, and the whole point of this path is throughput.
#[derive(Debug, Deserialize, ToSchema)]
pub struct IngestBatch {
    pub events: Vec<IngestEvent>,
}

/// Query string for GET /telemetry.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListTelemetryQuery {
    pub integration_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub status: Option<TelemetryStatus>,
    pub trace_id: Option<String>,
    /// Inclusive lower bound on occurred_at.
    pub from: Option<DateTime<Utc>>,
    /// Exclusive upper bound on occurred_at.
    pub to: Option<DateTime<Utc>>,
    /// Defaults to 100, capped at 1000.
    pub limit: Option<i64>,
}

/// Query string for GET /telemetry/summary.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SummaryQuery {
    pub integration_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

/// Query string for GET /telemetry/series.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SeriesQuery {
    pub integration_id: Option<Uuid>,
    pub environment_id: Option<Uuid>,
    /// Defaults to 24 hours ago.
    pub from: Option<DateTime<Utc>>,
    /// Defaults to now.
    pub to: Option<DateTime<Utc>>,
    /// Bucket width. Omit and one is chosen to give roughly 60 points, which
    /// is what a chart can actually render.
    pub bucket_seconds: Option<i64>,
}
