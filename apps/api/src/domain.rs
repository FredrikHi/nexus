//! Value types shared by more than one feature.
//!
//! Feature submodules are private, so one feature cannot reach into another's
//! `model`. Anything genuinely common lives here instead. Duplicating an enum
//! per feature would be worse than duplicating a function: the copies would
//! serialize identically yet be distinct, non-interchangeable Rust types.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Mirrors the `criticality` TEXT + CHECK column used by `systems` and
/// `integrations`. serde uses the exact DB tokens, so an unknown value in a
/// request body fails deserialization and Axum answers 422 for free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Criticality {
    Low,
    Medium,
    High,
    Critical,
}

impl Criticality {
    pub fn as_str(self) -> &'static str {
        match self {
            Criticality::Low => "LOW",
            Criticality::Medium => "MEDIUM",
            Criticality::High => "HIGH",
            Criticality::Critical => "CRITICAL",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "LOW" => Some(Criticality::Low),
            "MEDIUM" => Some(Criticality::Medium),
            "HIGH" => Some(Criticality::High),
            "CRITICAL" => Some(Criticality::Critical),
            _ => None,
        }
    }
}
