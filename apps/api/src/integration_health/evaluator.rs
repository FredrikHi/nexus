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
pub fn judge(
    policy: &ResolvedPolicy,
    m: &Measurement,
    previous: Option<HealthStatus>,
    now: DateTime<Utc>,
) -> Verdict {
    let stale_after = Duration::minutes(policy.stale_after_minutes as i64);

    let last_event_at = match m.last_event_at {
        // Never seen. This is the only honest UNKNOWN: there is nothing to
        // carry forward and nothing to judge.
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
                    "no telemetry for over {} hours; last event was at {}",
                    policy.stale_after_minutes / 60,
                    last.to_rfc3339()
                ),
            }
        }
        Some(last) => last,
    };

    // Quiet, but not quiet for long enough to doubt. An integration called a
    // few times a day is idle most of the time, and idle is not unknown: the
    // last thing we actually observed still stands.
    if m.event_count == 0 {
        return match previous {
            Some(status) if status != HealthStatus::Unknown => Verdict {
                status,
                reason: format!(
                    "no calls in the last {} minutes; still {} from the last one, at {}",
                    policy.window_minutes,
                    status.as_str().to_lowercase(),
                    last_event_at.to_rfc3339()
                ),
            },
            _ => Verdict {
                status: HealthStatus::Unknown,
                reason: format!(
                    "no calls in the last {} minutes and no earlier verdict to stand on",
                    policy.window_minutes
                ),
            },
        };
    }

    // Too few calls to trust a rate: one failure out of one would read as a
    // 100% error rate. But a handful of calls that all worked is evidence of
    // working, and a handful with a failure in them is worth showing rather
    // than hiding behind UNKNOWN.
    if m.event_count < policy.min_events as i64 {
        if m.error_count > 0 {
            return Verdict {
                status: HealthStatus::Degraded,
                reason: format!(
                    "{} of {} calls failed in the last {} minutes; too few to judge a rate",
                    m.error_count, m.event_count, policy.window_minutes
                ),
            };
        }

        return Verdict {
            status: HealthStatus::Healthy,
            reason: format!(
                "{} calls in the last {} minutes, none failed",
                m.event_count, policy.window_minutes
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
            // Twelve hours, matching the shipped default: a verdict is
            // meant to outlive a quiet afternoon.
            stale_after_minutes: 720,
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
        judge(&policy(), &m, None, Utc::now()).status
    }

    #[test]
    fn clean_traffic_is_healthy() {
        assert_eq!(
            status_of(measured(100, 0, Some(120.0))),
            HealthStatus::Healthy
        );
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
        assert_eq!(
            status_of(measured(100, 0, Some(2_500.0))),
            HealthStatus::Degraded
        );
        assert_eq!(
            status_of(measured(100, 0, Some(9_000.0))),
            HealthStatus::Unhealthy
        );
    }

    #[test]
    fn latency_is_ignored_when_the_policy_sets_no_limit() {
        let mut p = policy();
        p.p95_degraded_ms = None;
        p.p95_unhealthy_ms = None;
        let verdict = judge(&p, &measured(100, 0, Some(60_000.0)), None, Utc::now());
        assert_eq!(verdict.status, HealthStatus::Healthy);
    }

    #[test]
    fn a_failure_in_a_small_sample_degrades_rather_than_condemns() {
        // One failure out of one call is a 100% error rate, which means
        // nothing as a rate. It is still worth seeing, so it degrades rather
        // than being hidden behind UNKNOWN or treated as an outage.
        let verdict = judge(&policy(), &measured(1, 1, None), None, Utc::now());
        assert_eq!(verdict.status, HealthStatus::Degraded);
        assert!(
            verdict.reason.contains("too few to judge a rate"),
            "{}",
            verdict.reason
        );
    }

    #[test]
    fn a_small_clean_sample_is_healthy() {
        // Three calls, none failed. That is evidence of working, and the old
        // rule called it UNKNOWN.
        let verdict = judge(&policy(), &measured(3, 0, None), None, Utc::now());
        assert_eq!(verdict.status, HealthStatus::Healthy);
    }

    #[test]
    fn a_quiet_integration_keeps_the_verdict_it_earned() {
        // Three hours without a call, having been healthy. An integration used
        // a few times a day is idle most of the time, and idle is not unknown.
        let now = Utc::now();
        let m = Measurement {
            event_count: 0,
            error_count: 0,
            p95_duration_ms: None,
            last_event_at: Some(now - Duration::hours(3)),
        };
        let verdict = judge(&policy(), &m, Some(HealthStatus::Healthy), now);
        assert_eq!(verdict.status, HealthStatus::Healthy);
        assert!(
            verdict.reason.contains("still healthy"),
            "{}",
            verdict.reason
        );
    }

    #[test]
    fn a_quiet_integration_also_keeps_a_bad_verdict() {
        // Failing and then going quiet does not launder the failure.
        let now = Utc::now();
        let m = Measurement {
            event_count: 0,
            error_count: 0,
            p95_duration_ms: None,
            last_event_at: Some(now - Duration::hours(3)),
        };
        let verdict = judge(&policy(), &m, Some(HealthStatus::Unhealthy), now);
        assert_eq!(verdict.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn silence_with_nothing_to_carry_forward_is_unknown() {
        let now = Utc::now();
        let m = Measurement {
            event_count: 0,
            error_count: 0,
            p95_duration_ms: None,
            last_event_at: Some(now - Duration::hours(3)),
        };
        assert_eq!(
            judge(&policy(), &m, None, now).status,
            HealthStatus::Unknown
        );
    }

    #[test]
    fn a_verdict_does_not_outlive_the_stale_window() {
        // Thirteen hours is past the twelve the policy allows, so even a
        // healthy verdict stops standing behind it.
        let now = Utc::now();
        let m = Measurement {
            event_count: 0,
            error_count: 0,
            p95_duration_ms: None,
            last_event_at: Some(now - Duration::hours(13)),
        };
        let verdict = judge(&policy(), &m, Some(HealthStatus::Healthy), now);
        assert_eq!(verdict.status, HealthStatus::Unknown);
    }

    #[test]
    fn an_integration_that_never_reported_is_unknown() {
        let m = Measurement {
            event_count: 0,
            error_count: 0,
            p95_duration_ms: None,
            last_event_at: None,
        };
        let verdict = judge(&policy(), &m, None, Utc::now());
        assert_eq!(verdict.status, HealthStatus::Unknown);
        assert!(
            verdict.reason.contains("ever been recorded"),
            "{}",
            verdict.reason
        );
    }

    #[test]
    fn staleness_outranks_a_bad_error_rate() {
        // Old data must not keep an integration pinned to UNHEALTHY forever.
        let now = Utc::now();
        let m = Measurement {
            event_count: 100,
            error_count: 100,
            p95_duration_ms: None,
            last_event_at: Some(now - Duration::hours(14)),
        };
        assert_eq!(
            judge(&policy(), &m, Some(HealthStatus::Unhealthy), now).status,
            HealthStatus::Unknown
        );
    }
}
