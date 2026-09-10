use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::model::{IncidentStatus, Severity};

/// Query string for GET /incidents.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListIncidentsQuery {
    pub integration_id: Option<Uuid>,
    pub severity: Option<Severity>,
    pub status: Option<IncidentStatus>,
    /// Shorthand for everything not yet resolved, which is the usual view.
    pub only_open: Option<bool>,
    /// Defaults to 50, capped at 500.
    pub limit: Option<i64>,
}

/// Request body for adding a note to an incident's timeline.
///
/// There is no `actor` field: who did this comes from the verified token, so a
/// caller cannot attribute their note to somebody else.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AddNote {
    pub message: String,
}
