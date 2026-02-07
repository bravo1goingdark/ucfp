//! API resilience patterns: circuit breaker, rate limiting, and retry logic.

pub mod circuit_breaker;
pub mod rate_limit;
pub mod retry;

pub use circuit_breaker::{
    CircuitBreaker, CircuitBreakerConfig, CircuitBreakerManager, CircuitState,
};
pub use rate_limit::{RateLimitConfig, RateLimitManager, RateLimitStats, TokenBucket};
pub use retry::{
    execute_with_retry, execute_with_retry_async, is_retryable_error, RetryConfig, RetryResult,
};
