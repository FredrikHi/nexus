//! How long to wait before starting a crashed process again.

use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct RestartPolicy {
    /// Wait before the first restart.
    pub initial_delay: Duration,
    /// Longest wait between restarts, however many there have been.
    pub max_delay: Duration,
    /// A process that stayed up this long counts as having worked, so its next
    /// crash starts the delays over rather than continuing from the maximum.
    pub stable_after: Duration,
}

impl Default for RestartPolicy {
    /// Never gives up. A database that is down for an hour should find Nexus
    /// running again within a minute of it coming back, with nobody logging in
    /// to restart anything.
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            stable_after: Duration::from_secs(60),
        }
    }
}

/// Doubling delays, capped. A process crashing on start does not get restarted
/// in a tight loop that fills the log and burns a core.
#[derive(Debug)]
pub struct Backoff {
    policy: RestartPolicy,
    next: Duration,
}

impl Backoff {
    pub fn new(policy: RestartPolicy) -> Self {
        Self {
            policy,
            next: policy.initial_delay,
        }
    }

    pub fn next_delay(&mut self) -> Duration {
        let delay = self.next;
        self.next = (delay * 2).min(self.policy.max_delay);
        delay
    }

    pub fn reset(&mut self) {
        self.next = self.policy.initial_delay;
    }
}
