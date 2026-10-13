//! Radio-neutral connection states and bounded retry policy.

/// Observable network connection lifecycle shared by platform adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    /// No usable credentials have been provisioned.
    Unprovisioned,
    /// Credentials were loaded and validated but the radio has not started.
    Stored,
    /// The platform is initializing its radio and network stack.
    RadioStarting,
    /// The station is associating with its access point.
    Associating {
        /// One-based attempt number within the current bounded retry run.
        attempt: u8,
    },
    /// The station is associated and is acquiring an IP address.
    AcquiringAddress,
    /// The station has an IP address and can use routed sockets.
    GotIp,
    /// The bounded retry run ended without reaching `GotIp`.
    Failed,
}

/// Bounded exponential retry policy for a connection attempt sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryPolicy {
    max_attempts: u8,
    base_delay_millis: u32,
    max_delay_millis: u32,
}

impl RetryPolicy {
    /// Construct a retry policy.
    pub const fn new(max_attempts: u8, base_delay_millis: u32, max_delay_millis: u32) -> Self {
        Self {
            max_attempts,
            base_delay_millis,
            max_delay_millis,
        }
    }

    /// Return the maximum number of attempts, including the initial attempt.
    pub const fn max_attempts(self) -> u8 {
        self.max_attempts
    }

    /// Return the delay after a one-based failed attempt.
    ///
    /// `None` means the attempt budget is exhausted. The first failure uses
    /// the base delay, and subsequent failures double it up to the cap.
    pub const fn delay_after_failure(self, failed_attempt: u8) -> Option<u32> {
        if failed_attempt == 0 || failed_attempt >= self.max_attempts {
            return None;
        }
        let shift = if failed_attempt - 1 < 31 {
            failed_attempt - 1
        } else {
            31
        };
        let multiplier = 1_u32 << shift;
        let delay = self.base_delay_millis.saturating_mul(multiplier);
        Some(if delay < self.max_delay_millis {
            delay
        } else {
            self.max_delay_millis
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_is_exponential_capped_and_bounded() {
        let policy = RetryPolicy::new(6, 250, 2_000);
        assert_eq!(policy.delay_after_failure(1), Some(250));
        assert_eq!(policy.delay_after_failure(2), Some(500));
        assert_eq!(policy.delay_after_failure(3), Some(1_000));
        assert_eq!(policy.delay_after_failure(4), Some(2_000));
        assert_eq!(policy.delay_after_failure(5), Some(2_000));
        assert_eq!(policy.delay_after_failure(6), None);
    }
}
