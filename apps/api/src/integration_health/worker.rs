//! The background evaluator.
//!
//! Health cannot be computed on read the way traces are. A status change has
//! to be noticed while nobody is watching, because that transition is what an
//! incident gets opened from. So this runs on a timer instead.

use std::time::{Duration, Instant};

use sqlx::PgPool;

use super::{repository, service};

/// Identifies this particular job to Postgres' advisory lock manager. Any
/// constant works as long as nothing else in the database picks the same one.
const EVALUATION_LOCK: i64 = 0x494F_5048; // "IOPH"

/// How long between retention passes. Dropping a partition is cheap, but there
/// is no point looking more than a few times a day.
const PRUNE_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

pub struct WorkerConfig {
    pub interval: Duration,
    pub retention_days: i64,
}

impl WorkerConfig {
    /// Reads configuration once, at startup, like the rest of the app.
    pub fn from_env() -> Self {
        let interval_secs = std::env::var("HEALTH_EVAL_INTERVAL_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60)
            .max(5);

        let retention_days = std::env::var("TELEMETRY_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(90)
            .max(1);

        WorkerConfig { interval: Duration::from_secs(interval_secs), retention_days }
    }
}

/// Whether the worker should run at all. Off is useful for a one-shot process
/// or a second instance you want serving traffic only.
pub fn enabled() -> bool {
    !matches!(
        std::env::var("HEALTH_WORKER_ENABLED").as_deref(),
        Ok("false") | Ok("0")
    )
}

/// Starts the loop on the Tokio runtime and returns immediately.
///
/// The task outlives the call and stops when the process does. It never
/// returns an error: a failed pass is logged and the next tick tries again,
/// because a transient database blip must not silently stop health evaluation
/// for the lifetime of the process.
pub fn spawn(db: PgPool, config: WorkerConfig) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(config.interval);
        // If a pass overruns, skip the missed ticks rather than queueing them:
        // catching up on stale evaluations has no value.
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut last_prune: Option<Instant> = None;

        loop {
            ticker.tick().await;

            match run_once(&db, &config, &mut last_prune).await {
                Ok(Some(summary)) => tracing::debug!("{summary}"),
                // Another instance held the lock; nothing to do.
                Ok(None) => tracing::trace!("health evaluation skipped, lock held elsewhere"),
                Err(err) => tracing::error!(error = %err, "health evaluation pass failed"),
            }
        }
    });
}

/// One pass, guarded so that only one instance evaluates at a time.
///
/// Without the advisory lock, two API instances would both evaluate, both see
/// the same transition, and both write it: duplicate history, and later
/// duplicate incidents. The lock is held on a single connection and released
/// explicitly; if the process dies the connection closes and Postgres releases
/// it anyway, so a crash cannot wedge the job forever.
async fn run_once(
    db: &PgPool,
    config: &WorkerConfig,
    last_prune: &mut Option<Instant>,
) -> anyhow::Result<Option<String>> {
    let mut conn = db.acquire().await?;

    let acquired: bool = sqlx::query_scalar!(
        r#"SELECT pg_try_advisory_lock($1) AS "acquired!""#,
        EVALUATION_LOCK
    )
    .fetch_one(&mut *conn)
    .await?;

    if !acquired {
        return Ok(None);
    }

    // Run the work, but release the lock whatever happens.
    let outcome = evaluate_and_prune(db, config, last_prune).await;

    sqlx::query!("SELECT pg_advisory_unlock($1)", EVALUATION_LOCK)
        .fetch_one(&mut *conn)
        .await?;

    outcome.map(Some)
}

async fn evaluate_and_prune(
    db: &PgPool,
    config: &WorkerConfig,
    last_prune: &mut Option<Instant>,
) -> anyhow::Result<String> {
    let result = service::evaluate_default_org(db).await?;

    // Reconcile immediately, in the same locked pass. Incidents are derived
    // from the health state this just wrote, so doing it here means the two
    // never sit out of step for a whole interval.
    let incidents = crate::incidents::reconcile_default_org(db).await?;

    let mut summary = format!(
        "evaluated {} integrations, {} changed status; incidents +{} ~{} -{}",
        result.evaluated, result.changed,
        incidents.opened, incidents.escalated, incidents.resolved
    );

    let due = last_prune.map_or(true, |t| t.elapsed() >= PRUNE_EVERY);
    if due {
        let dropped = repository::prune_telemetry(db, config.retention_days).await?;
        *last_prune = Some(Instant::now());
        if dropped > 0 {
            tracing::info!(
                dropped,
                retention_days = config.retention_days,
                "dropped expired telemetry partitions"
            );
        }
        summary.push_str(&format!("; pruned {dropped} telemetry partitions"));
    }

    Ok(summary)
}
