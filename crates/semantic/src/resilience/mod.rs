//! API resilience patterns: circuit breaker, rate limiting, and retry logic.
//!
//! These handle transient failures
//! and prevent cascading failures when external services are overloaded.

mod circuit_breaker;
mod rate_limit;
mod retry;

pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerManager, CircuitState,
};
pub use rate_limit::{RateLimitConfig, RateLimitManager, RateLimitStats, TokenBucket};
pub use retry::{
    execute_with_retry, execute_with_retry_async, is_retryable_error, RetryConfig, RetryResult,
};
