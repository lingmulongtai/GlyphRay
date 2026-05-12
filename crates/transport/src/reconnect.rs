use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub max_attempts: u32,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(150),
            max_delay: Duration::from_secs(5),
            max_attempts: 12,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectState {
    policy: ReconnectPolicy,
    attempts: u32,
}

impl ReconnectState {
    pub fn new(policy: ReconnectPolicy) -> Self {
        Self {
            policy,
            attempts: 0,
        }
    }

    pub fn reset(&mut self) {
        self.attempts = 0;
    }

    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.attempts >= self.policy.max_attempts {
            return None;
        }
        let multiplier = 1_u32.checked_shl(self.attempts.min(10)).unwrap_or(u32::MAX);
        self.attempts += 1;
        Some((self.policy.initial_delay * multiplier).min(self.policy.max_delay))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconnect_backoff_caps_at_max_delay() {
        let mut state = ReconnectState::new(ReconnectPolicy {
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(250),
            max_attempts: 4,
        });

        assert_eq!(state.next_delay(), Some(Duration::from_millis(100)));
        assert_eq!(state.next_delay(), Some(Duration::from_millis(200)));
        assert_eq!(state.next_delay(), Some(Duration::from_millis(250)));
        assert_eq!(state.next_delay(), Some(Duration::from_millis(250)));
        assert_eq!(state.next_delay(), None);
    }
}
