//! Circuit breaker pattern for API resilience.
//!
//! Prevents cascading failures by temporarily disabling requests to failing services.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// States of the circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CircuitState {
    /// Normal operation - requests allowed.
    Closed,
    /// Failing fast - requests immediately rejected.
    Open,
    /// Testing if service recovered - limited requests allowed.
    HalfOpen,
}

/// Configuration for circuit breaker behavior.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CircuitBreakerConfig {
    /// Number of failures before opening circuit.
    pub failure_threshold: u32,
    /// Duration to wait before attempting recovery (Half-Open) in milliseconds.
    #[serde(with = "crate::serde_millis")]
    pub reset_timeout: Duration,
    /// Number of successes required in Half-Open to close circuit.
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(60),
            success_threshold: 2,
        }
    }
}

impl CircuitBreakerConfig {
    pub fn with_failure_threshold(mut self, threshold: u32) -> Self {
        self.failure_threshold = threshold;
        self
    }

    pub fn with_reset_timeout(mut self, timeout: Duration) -> Self {
        self.reset_timeout = timeout;
        self
    }

    pub fn with_success_threshold(mut self, threshold: u32) -> Self {
        self.success_threshold = threshold;
        self
    }
}

/// Circuit breaker for a single service/provider.
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: Mutex<CircuitState>,
    failure_count: AtomicU64,
    success_count: AtomicU64,
    last_failure_time: Mutex<Option<Instant>>,
    last_state_change: Mutex<Instant>,
}

impl CircuitBreaker {
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

    /// Check if request is allowed through.
    pub fn allow_request(&self) -> bool {
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if reset timeout has elapsed
                let last_change = *self.last_state_change.lock().unwrap();
                if last_change.elapsed() >= self.config.reset_timeout {
                    *state = CircuitState::HalfOpen;
                    *self.last_state_change.lock().unwrap() = Instant::now();
                    self.success_count.store(0, Ordering::SeqCst);
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
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::HalfOpen => {
                let successes = self.success_count.fetch_add(1, Ordering::SeqCst) + 1;
                if successes >= self.config.success_threshold as u64 {
                    *state = CircuitState::Closed;
                    *self.last_state_change.lock().unwrap() = Instant::now();
                    self.failure_count.store(0, Ordering::SeqCst);
                    self.success_count.store(0, Ordering::SeqCst);
                }
            }
            CircuitState::Closed => {
                // Reset failure count on success in closed state
                self.failure_count.store(0, Ordering::SeqCst);
            }
            _ => {}
        }
    }

    /// Record a failed request.
    pub fn record_failure(&self) {
        let mut state = self.state.lock().unwrap();

        match *state {
            CircuitState::HalfOpen => {
                // Any failure in half-open immediately opens circuit
                *state = CircuitState::Open;
                *self.last_state_change.lock().unwrap() = Instant::now();
                self.failure_count.fetch_add(1, Ordering::SeqCst);
            }
            CircuitState::Closed => {
                let failures = self.failure_count.fetch_add(1, Ordering::SeqCst) + 1;
                *self.last_failure_time.lock().unwrap() = Some(Instant::now());

                if failures >= self.config.failure_threshold as u64 {
                    *state = CircuitState::Open;
                    *self.last_state_change.lock().unwrap() = Instant::now();
                }
            }
            _ => {}
        }
    }

    /// Get current state (for monitoring).
    pub fn current_state(&self) -> CircuitState {
        *self.state.lock().unwrap()
    }

    /// Get failure count.
    pub fn failure_count(&self) -> u64 {
        self.failure_count.load(Ordering::SeqCst)
    }

    /// Get time since last state change.
    pub fn time_in_current_state(&self) -> Duration {
        self.last_state_change.lock().unwrap().elapsed()
    }
}

/// Manager for multiple circuit breakers (one per provider).
#[derive(Debug)]
pub struct CircuitBreakerManager {
    breakers: dashmap::DashMap<String, Arc<CircuitBreaker>>,
    default_config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    pub fn new(default_config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: dashmap::DashMap::new(),
            default_config,
        }
    }

    /// Get or create circuit breaker for a provider.
    pub fn get_or_create(&self, provider: &str) -> Arc<CircuitBreaker> {
        self.breakers
            .entry(provider.to_string())
            .or_insert_with(|| Arc::new(CircuitBreaker::new(self.default_config)))
            .clone()
    }

    /// Get state of a specific provider.
    pub fn get_state(&self, provider: &str) -> Option<CircuitState> {
        self.breakers.get(provider).map(|b| b.current_state())
    }

    /// Check if provider is healthy (circuit closed or half-open).
    pub fn is_healthy(&self, provider: &str) -> bool {
        self.get_state(provider)
            .map(|s| s != CircuitState::Open)
            .unwrap_or(true)
    }

    /// Reset all circuit breakers (useful for admin operations).
    pub fn reset_all(&self) {
        self.breakers.clear();
    }

    /// Get stats for all providers.
    pub fn get_all_stats(&self) -> Vec<(String, CircuitState, u64)> {
        self.breakers
            .iter()
            .map(|entry| {
                let (name, breaker) = entry.pair();
                (
                    name.clone(),
                    breaker.current_state(),
                    breaker.failure_count(),
                )
            })
            .collect()
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
    fn test_circuit_breaker_starts_closed() {
        let cb = CircuitBreaker::new(CircuitBreakerConfig::default());
        assert_eq!(cb.current_state(), CircuitState::Closed);
        assert!(cb.allow_request());
    }

    #[test]
    fn test_circuit_opens_after_failures() {
        let config = CircuitBreakerConfig::default().with_failure_threshold(3);
        let cb = CircuitBreaker::new(config);

        // Record failures
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.current_state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.current_state(), CircuitState::Open);
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_circuit_closes_on_successes() {
        let config = CircuitBreakerConfig::default()
            .with_failure_threshold(1)
            .with_reset_timeout(Duration::from_millis(0))
            .with_success_threshold(2);
        let cb = CircuitBreaker::new(config);

        // Open the circuit
        cb.record_failure();
        assert_eq!(cb.current_state(), CircuitState::Open);

        // Should transition to half-open immediately (0 timeout)
        assert!(cb.allow_request());
        assert_eq!(cb.current_state(), CircuitState::HalfOpen);

        // Record successes to close
        cb.record_success();
        assert_eq!(cb.current_state(), CircuitState::HalfOpen);
        cb.record_success();
        assert_eq!(cb.current_state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_fails_immediately() {
        let config = CircuitBreakerConfig::default()
            .with_failure_threshold(1)
            .with_reset_timeout(Duration::from_millis(0));
        let cb = CircuitBreaker::new(config);

        cb.record_failure();
        assert!(cb.allow_request()); // Half-open
        cb.record_failure();
        assert_eq!(cb.current_state(), CircuitState::Open);
    }

    #[test]
    fn test_manager_creates_breakers() {
        let manager = CircuitBreakerManager::default();
        let cb = manager.get_or_create("openai");

        assert!(manager.is_healthy("openai"));
        assert_eq!(manager.get_state("openai"), Some(CircuitState::Closed));

        // Record failure
        cb.record_failure();
        assert_eq!(cb.failure_count(), 1);
    }
}
