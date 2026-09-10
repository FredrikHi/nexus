//! Turning a measurement into a verdict.
//!
//! Deliberately a pure function over plain values: no database, no async, no
//! clock of its own. Every rule below is therefore testable in microseconds,
//! which matters because these thresholds are the part users will argue with.

use chrono::{DateTime, Duration, Utc};

use super::model::{HealthStatus, Measurement, ResolvedPolicy, Verdict};

/// Judges one integration.
///
/// Thresholds are exclusive: a rate must EXCEED the threshold to count, not
/// merely equal it. That reading makes "degraded above 5%" mean what it says,
/// and it avoids the trap where a threshold of zero would condemn a perfectly
/// healthy integration with no errors at all.
///
/// Order matters. Staleness is checked first because an integration with no
/// recent traffic cannot be judged on rates at all, and the unhealthy
/// thresholds are checked before the degraded ones so the worse verdict wins.
pub fn judge(policy: &ResolvedPolicy, m: &Measurement, now: DateTime<Utc>) -> Verdict {
    let stale_after = Duration::minutes(policy.stale_after_minutes as i64);

    // No traffic at all, or none recently. Silence is not failure.
    match m.last_event_at {
        None => {
            return Verdict {
                status: HealthStatus::Unknown,
                reason: "no telemetry has ever been recorded for this integration".to_string(),
            }
        }
        Some(last) if now - last > stale_after => {
            return Verdict {
                status: HealthStatus::Unknown,
                reason: format!(
                    "no telemetry in the last {} minutes; last event was at {}",
                    policy.stale_after_minutes,
                    last.to_rfc3339()
                ),
            }
        }
        Some(_) => {}
    }

    // Too little traffic to draw a conclusion. Without this, one failure out of
    // one call reads as a 100% error rate and declares an outage.
    if m.event_count < policy.min_events as i64 {
        return Verdict {
            status: HealthStatus::Unknown,
            reason: format!(
                "only {} events in the last {} minutes; {} are needed to judge",
                m.event_count, policy.window_minutes, policy.min_events
            ),
        };
    }

    let rate = m.error_rate();

    if rate > policy.error_rate_unhealthy {
        return Verdict {
            status: HealthStatus::Unhealthy,
            reason: format!(
                "error rate {:.1}% over {} events exceeds the unhealthy threshold of {:.1}%",
                rate * 100.0,
                m.event_count,
                policy.error_rate_unhealthy * 100.0
            ),
        };
    }

    if let (Some(p95), Some(limit)) = (m.p95_duration_ms, policy.p95_unhealthy_ms) {
        if p95 > limit as f64 {
            return Verdict {
                status: HealthStatus::Unhealthy,
                reason: format!(
                    "p95 latency {p95:.0}ms exceeds the unhealthy threshold of {limit}ms"
                ),
            };
        }
    }

    if rate > policy.error_rate_degraded {
        return Verdict {
            status: HealthStatus::Degraded,
            reason: format!(
                "error rate {:.1}% over {} events exceeds the degraded threshold of {:.1}%",
                rate * 100.0,
                m.event_count,
                policy.error_rate_degraded * 100.0
            ),
        };
    }

    if let (Some(p95), Some(limit)) = (m.p95_duration_ms, policy.p95_degraded_ms) {
        if p95 > limit as f64 {
            return Verdict {
                status: HealthStatus::Degraded,
                reason: format!(
                    "p95 latency {p95:.0}ms exceeds the degraded threshold of {limit}ms"
                ),
            };
        }
    }

    Verdict {
        status: HealthStatus::Healthy,
        reason: format!(
            "error rate {:.1}% over {} events in the last {} minutes",
            rate * 100.0,
            m.event_count,
            policy.window_minutes
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> ResolvedPolicy {
        ResolvedPolicy {
            window_minutes: 15,
            min_events: 5,
            error_rate_degraded: 0.05,
            error_rate_unhealthy: 0.20,
            p95_degraded_ms: Some(1_000),
            p95_unhealthy_ms: Some(5_000),
            stale_after_minutes: 60,
        }
    }

    fn measured(event_count: i64, error_count: i64, p95: Option<f64>) -> Measurement {
        Measurement {
            event_count,
            error_count,
            p95_duration_ms: p95,
            last_event_at: Some(Utc::now()),
        }
    }

    fn status_of(m: Measurement) -> HealthStatus {
        judge(&policy(), &m, Utc::now()).status
    }

    #[test]
    fn clean_traffic_is_healthy() {
        assert_eq!(status_of(measured(100, 0, Some(120.0))), HealthStatus::Healthy);
    }

    #[test]
    fn a_rate_equal_to_the_threshold_is_not_yet_degraded() {
        // 5 errors in 100 is exactly 5%, and the threshold is exclusive.
        assert_eq!(status_of(measured(100, 5, None)), HealthStatus::Healthy);
    }

    #[test]
    fn exceeding_the_degraded_threshold_degrades() {
        assert_eq!(status_of(measured(100, 6, None)), HealthStatus::Degraded);
    }

    #[test]
    fn exceeding_the_unhealthy_threshold_wins_over_degraded() {
        assert_eq!(status_of(measured(100, 40, None)), HealthStatus::Unhealthy);
    }

    #[test]
    fn latency_alone_can_degrade_a_error_free_integration() {
        assert_eq!(status_of(measured(100, 0, Some(2_500.0))), HealthStatus::Degraded);
        assert_eq!(status_of(measured(100, 0, Some(9_000.0))), HealthStatus::Unhealthy);
    }

    #[test]
    fn latency_is_ignored_when_the_policy_sets_no_limit() {
        let mut p = policy();
        p.p95_degraded_ms = None;
        p.p95_unhealthy_ms = None;
        let verdict = judge(&p, &measured(100, 0, Some(60_000.0)), Utc::now());
        assert_eq!(verdict.status, HealthStatus::Healthy);
    }

    #[test]
    fn too_few_events_is_unknown_not_unhealthy() {
        // One failure out of one call is a 100% error rate, and means nothing.
        let verdict = judge(&policy(), &measured(1, 1, None), Utc::now());
        assert_eq!(verdict.status, HealthStatus::Unknown);
        assert!(verdict.reason.contains("needed to judge"), "{}", verdict.reason);
    }

    #[test]
    fn silence_is_unknown_not_unhealthy() {
        let now = Utc::now();
        let m = Measurement {
            event_count: 0,
            error_count: 0,
            p95_duration_ms: None,
            last_event_at: Some(now - Duration::hours(3)),
        };
        assert_eq!(judge(&policy(), &m, now).status, HealthStatus::Unknown);
    }

    #[test]
    fn an_integration_that_never_reported_is_unknown() {
        let m = Measurement {
            event_count: 0,
            error_count: 0,
            p95_duration_ms: None,
            last_event_at: None,
        };
        let verdict = judge(&policy(), &m, Utc::now());
        assert_eq!(verdict.status, HealthStatus::Unknown);
        assert!(verdict.reason.contains("ever been recorded"), "{}", verdict.reason);
    }

    #[test]
    fn staleness_outranks_a_bad_error_rate() {
        // Old data must not keep an integration pinned to UNHEALTHY forever.
        let now = Utc::now();
        let m = Measurement {
            event_count: 100,
            error_count: 100,
            p95_duration_ms: None,
            last_event_at: Some(now - Duration::hours(6)),
        };
        assert_eq!(judge(&policy(), &m, now).status, HealthStatus::Unknown);
    }
}
