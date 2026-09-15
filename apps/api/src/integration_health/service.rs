use std::collections::HashMap;

use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::ApiError;

use super::evaluator;
use super::model::{EvaluationResult, HealthStatus, HealthTransition, IntegrationHealth};
use super::repository;

const DEFAULT_TRANSITION_LIMIT: i64 = 50;
const MAX_TRANSITION_LIMIT: i64 = 500;

/// Evaluates every monitored integration in one organization.
///
/// Runs on a schedule rather than on read, because a status change has to be
/// noticed when nobody is looking: that transition is what an incident will be
/// opened from.
pub async fn evaluate(db: &PgPool, org_id: Uuid) -> Result<EvaluationResult, ApiError> {
    let inputs = repository::gather(db, org_id).await?;

    // One query for the previous statuses rather than one per integration.
    let previous: HashMap<Uuid, String> = repository::current_statuses(db, org_id)
        .await?
        .into_iter()
        .collect();

    let now = Utc::now();
    let mut changed = 0usize;

    for input in &inputs {
        let before = previous.get(&input.integration_id).map(String::as_str);
        // The last verdict is an input now, not just something to compare
        // against: a quiet integration keeps the status it earned.
        let verdict = evaluator::judge(
            &input.policy,
            &input.measurement,
            before.and_then(HealthStatus::from_db),
            now,
        );

        repository::record(db, input, &verdict).await?;

        // A transition is recorded on a real change, and on the first ever
        // evaluation, where `before` is None.
        if before != Some(verdict.status.as_str()) {
            repository::record_transition(db, input, before, &verdict).await?;
            changed += 1;
        }
    }

    Ok(EvaluationResult {
        evaluated: inputs.len(),
        changed,
    })
}

/// Evaluates every organization on the platform.
///
/// The worker sweeps all tenants rather than one: a hosted deployment has
/// many, and health that is only computed for whoever happens to be looking is
/// not health at all.
pub async fn evaluate_all(db: &PgPool) -> Result<EvaluationResult, ApiError> {
    let mut total = EvaluationResult {
        evaluated: 0,
        changed: 0,
    };

    for org_id in crate::organizations::all_ids(db).await? {
        let result = evaluate(db, org_id).await?;
        total.evaluated += result.evaluated;
        total.changed += result.changed;
    }

    Ok(total)
}

pub async fn list(db: &PgPool, org_id: Uuid) -> Result<Vec<IntegrationHealth>, ApiError> {
    repository::list(db, org_id)
        .await?
        .into_iter()
        .map(|r| r.into_domain())
        .collect()
}

pub async fn get(
    db: &PgPool,
    org_id: Uuid,
    integration_id: Uuid,
) -> Result<IntegrationHealth, ApiError> {
    repository::find(db, org_id, integration_id)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!(
                "integration {integration_id} has no health record yet; \
                 it may be new, unmonitored, or not evaluated since it was created"
            ))
        })?
        .into_domain()
}

pub async fn transitions(
    db: &PgPool,
    org_id: Uuid,
    integration_id: Uuid,
    limit: Option<i64>,
) -> Result<Vec<HealthTransition>, ApiError> {
    let limit = limit
        .unwrap_or(DEFAULT_TRANSITION_LIMIT)
        .clamp(1, MAX_TRANSITION_LIMIT);

    repository::transitions(db, org_id, integration_id, limit)
        .await?
        .into_iter()
        .map(|r| r.into_domain())
        .collect()
}

/// Counts of each status, for a dashboard header.
pub async fn overview(db: &PgPool, org_id: Uuid) -> Result<HashMap<&'static str, usize>, ApiError> {
    let mut counts: HashMap<&'static str, usize> = HashMap::new();
    for status in [
        HealthStatus::Healthy,
        HealthStatus::Degraded,
        HealthStatus::Unhealthy,
        HealthStatus::Unknown,
    ] {
        counts.insert(status.as_str(), 0);
    }

    for health in list(db, org_id).await? {
        *counts.entry(health.status.as_str()).or_insert(0) += 1;
    }

    Ok(counts)
}
