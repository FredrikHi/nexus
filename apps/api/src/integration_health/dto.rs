use serde::Deserialize;
use utoipa::IntoParams;

/// Query string for the transition log.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TransitionsQuery {
    /// Defaults to 50, capped at 500.
    pub limit: Option<i64>,
}
