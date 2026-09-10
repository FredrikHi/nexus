use chrono::{DateTime, Utc};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

/// Query string for GET /traces.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListTracesQuery {
    /// Traces that touch this integration anywhere along the flow, not just
    /// the spans belonging to it.
    pub integration_id: Option<Uuid>,
    /// Inclusive lower bound on span time.
    pub from: Option<DateTime<Utc>>,
    /// Exclusive upper bound on span time.
    pub to: Option<DateTime<Utc>>,
    /// When true, only traces containing a FAILURE or TIMEOUT span.
    pub only_errors: Option<bool>,
    /// Defaults to 50, capped at 500.
    pub limit: Option<i64>,
}
