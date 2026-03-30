//! Circuit breaker pattern for API resilience.
//!
//! The circuit breaker prevents cascading failures by stopping requests to a failing service
//! after a threshold of failures is reached. It periodically attempts to reset (half-open state)
//! to check if the service has recovered.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Configuration for circuit breaker behavior.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Duration to wait before attempting to close the circuit (half-open).
    pub reset_timeout: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(30),
        }
    }
}

impl CircuitBreakerConfig {
    /// Create a new config with custom failure threshold.
    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    /// Create a new config with custom reset timeout.
    pub fn with_reset_timeout(mut self, timeout: Duration) -> Self {
        self.reset_timeout = timeout;
        self
    }
}

/// Current state of the circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests are allowed.
    Closed,
    /// Circuit is open, requests are rejected.
    Open,
    /// Circuit is half-open, allowing test requests.
    HalfOpen,
}

/// All mutable state for the circuit breaker, behind a single mutex.
struct CircuitBreakerInner {
    state: CircuitState,
    failure_count: u64,
    success_count: u64,
    last_failure_time: Option<Instant>,
    last_state_change: Instant,
}

/// Circuit breaker implementation for preventing cascading failures.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    inner: Mutex<CircuitBreakerInner>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            inner: Mutex::new(CircuitBreakerInner {
                state: CircuitState::Closed,
                failure_count: 0,
                success_count: 0,
                last_failure_time: None,
                last_state_change: Instant::now(),
            }),
        }
    }

    /// Check if a request should be allowed through.
    pub fn allow_request(&self) -> bool {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        match inner.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if inner.last_state_change.elapsed() >= self.config.reset_timeout {
                    inner.state = CircuitState::HalfOpen;
                    inner.last_state_change = Instant::now();
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful request.
    pub fn record_success(&self) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.success_count += 1;

        match inner.state {
            CircuitState::HalfOpen => {
                inner.state = CircuitState::Closed;
                inner.failure_count = 0;
                inner.last_state_change = Instant::now();
            }
            CircuitState::Closed => {
                inner.failure_count = 0;
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.failure_count += 1;
        inner.last_failure_time = Some(Instant::now());

        match inner.state {
            CircuitState::Closed => {
                if inner.failure_count >= self.config.failure_threshold as u64 {
                    inner.state = CircuitState::Open;
                    inner.last_state_change = Instant::now();
                }
            }
            CircuitState::HalfOpen => {
                inner.state = CircuitState::Open;
                inner.last_state_change = Instant::now();
            }
            CircuitState::Open => {}
        }
    }

    /// Get the current state.
    pub fn current_state(&self) -> CircuitState {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .state
    }

    /// Get failure count.
    pub fn failure_count(&self) -> u64 {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .failure_count
    }

    /// Get success count.
    pub fn success_count(&self) -> u64 {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .success_count
    }
}

/// Manager for multiple circuit breakers (one per provider).
pub struct CircuitBreakerManager {
    breakers: Mutex<std::collections::HashMap<String, Arc<CircuitBreaker>>>,
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    /// Create a new manager with default config.
    pub fn new(default_config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: Mutex::new(std::collections::HashMap::new()),
            default_config,
        }
    }

    /// Get or create a circuit breaker for a provider.
    pub fn get_or_create(&self, provider: &str) -> Arc<CircuitBreaker> {
        let mut breakers = self
            .breakers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        breakers
            .entry(provider.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::new(self.default_config)))
            .clone()
    }

    /// Get all circuit breaker stats.
    pub fn get_all_stats(&self) -> Vec<(String, CircuitState, u64)> {
        let breakers = self
            .breakers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        breakers
            .iter()
            .map(|(name, cb)| (name.clone(), cb.current_state(), cb.failure_count()))
            .collect()
    }

    /// Reset all circuit breakers.
    pub fn reset_all(&self) {
        let mut breakers = self
            .breakers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        breakers.clear();
    }

    /// Check if a provider is healthy (circuit closed).
    pub fn is_healthy(&self, provider: &str) -> bool {
        let breakers = self
            .breakers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        breakers
            .get(provider)
            .map(|cb| cb.current_state() == CircuitState::Closed)
            .unwrap_or(true)
    }
}

impl Default for CircuitBreakerManager {
    fn default() -> Self {
        Self::new(CircuitBreakerConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        assert_eq!(cb.current_state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn circuit_breaker_opens_after_failures() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default().with_failure_threshold(3));

        // Record 3 failures
        for _ in 0..3 {
            cb.record_failure();
        }

        assert_eq!(cb.current_state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn circuit_breaker_resets_on_success() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default().with_failure_threshold(3));

        // Record 2 failures
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.current_state(), CircuitState::Closed);

        // Success resets counter
        cb.record_success();

        // Now 3 more failures needed to open
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.current_state(), CircuitState::Open);
    }

    #[test]
    fn circuit_breaker_manager_tracks_multiple() {
        let manager = CircuitBreakerManager::default();

        let cb1 = manager.get_or_create("provider1");
        let cb2 = manager.get_or_create("provider2");

        // Different providers should have different breakers
        cb1.record_failure();
        cb1.record_failure();
        cb1.record_failure();
        cb1.record_failure();
        cb1.record_failure();

        assert_eq!(cb1.current_state(), CircuitState::Open);
        assert_eq!(cb2.current_state(), CircuitState::Closed); // Still closed
    }
}
