//! Circuit breaker pattern for API resilience.
//!
//! The circuit breaker prevents cascading failures by stopping requests to a failing service
//! after a threshold of failures is reached. It periodically attempts to reset (half-open state)
//! to check if the service has recovered.

use std::sync::atomic::{AtomicU64, Ordering};
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

/// Circuit breaker implementation for preventing cascading failures.
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Mutex<CircuitState>,
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: Mutex<Option<Instant>>,
    last_state_change: Mutex<Instant>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given configuration.
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: Mutex::new(CircuitState::Closed),
            failure_count: AtomicU64::new(0),
            success_count: AtomicU64::new(0),
            last_failure_time: Mutex::new(None),
            last_state_change: Mutex::new(Instant::now()),
        }
    }

    /// Check if a request should be allowed through.
    pub fn allow_request(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                let last_change = *self.last_state_change.lock().unwrap();
                if last_change.elapsed() >= self.config.reset_timeout {
                    *state = CircuitState::HalfOpen;
                    *self.last_state_change.lock().unwrap() = Instant::now();
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
        self.success_count.fetch_add(1, Ordering::Relaxed);

        let mut state = self.state.lock().unwrap();
        match *state {
            CircuitState::HalfOpen => {
                // Transition back to closed after success in half-open
                *state = CircuitState::Closed;
                self.failure_count.store(0, Ordering::Relaxed);
                *self.last_state_change.lock().unwrap() = Instant::now();
            }
            CircuitState::Closed => {
                // Reset failure count on success
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        *self.last_failure_time.lock().unwrap() = Some(Instant::now());

        let mut state = self.state.lock().unwrap();
        match *state {
            CircuitState::Closed => {
                if failures >= self.config.failure_threshold as u64 {
                    *state = CircuitState::Open;
                    *self.last_state_change.lock().unwrap() = Instant::now();
                }
            }
            CircuitState::HalfOpen => {
                // Transition back to open on failure in half-open
                *state = CircuitState::Open;
                *self.last_state_change.lock().unwrap() = Instant::now();
            }
            CircuitState::Open => {}
        }
    }

    /// Get the current state.
    pub fn current_state(&self) -> CircuitState {
        *self.state.lock().unwrap()
    }

    /// Get failure count.
    pub fn failure_count(&self) -> u64 {
        self.failure_count.load(Ordering::Relaxed)
    }

    /// Get success count.
    pub fn success_count(&self) -> u64 {
        self.success_count.load(Ordering::Relaxed)
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
        let mut breakers = self.breakers.lock().unwrap();
        breakers
            .entry(provider.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::new(self.default_config)))
            .clone()
    }

    /// Get all circuit breaker stats.
    pub fn get_all_stats(&self) -> Vec<(String, CircuitState, u64)> {
        let breakers = self.breakers.lock().unwrap();
        breakers
            .iter()
            .map(|(name, cb)| (name.clone(), cb.current_state(), cb.failure_count()))
            .collect()
    }

    /// Reset all circuit breakers.
    pub fn reset_all(&self) {
        let mut breakers = self.breakers.lock().unwrap();
        breakers.clear();
    }

    /// Check if a provider is healthy (circuit closed).
    pub fn is_healthy(&self, provider: &str) -> bool {
        let breakers = self.breakers.lock().unwrap();
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
