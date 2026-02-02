//! Retry logic with exponential backoff for API calls.
//!
//! Provides configurable retry policies for handling transient failures.
//! Supports both synchronous and asynchronous execution.

use serde::{Deserialize, Serialize};
use std::thread;
use std::time::Duration;
use tokio::time::sleep;

/// Configuration for retry behavior.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Initial delay between retries (base for exponential backoff) in milliseconds.
    #[serde(with = "crate::serde_millis")]
    pub base_delay: Duration,
    /// Maximum delay between retries in milliseconds.
    #[serde(with = "crate::serde_millis")]
    pub max_delay: Duration,
    /// Multiplier for exponential backoff.
    pub backoff_multiplier: f64,
    /// Add random jitter to prevent thundering herd.
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn with_base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    pub fn with_backoff_multiplier(mut self, multiplier: f64) -> Self {
        self.backoff_multiplier = multiplier;
        self
    }

    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }

    /// Calculate delay for a specific retry attempt (0-indexed).
    pub fn calculate_delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }

        // Exponential backoff: base_delay * multiplier^(attempt-1)
        let exponential =
            self.base_delay.as_millis() as f64 * self.backoff_multiplier.powi((attempt - 1) as i32);

        let delay_ms = exponential.min(self.max_delay.as_millis() as f64) as u64;

        // Add jitter (±25%) to prevent synchronized retries
        if self.jitter {
            let jitter_range = delay_ms / 4;
            if jitter_range > 0 {
                // Simple pseudo-random based on current time nanos
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as u64;
                let jitter = nanos % (jitter_range * 2);
                let delay_with_jitter = delay_ms.saturating_sub(jitter_range) + jitter;
                return Duration::from_millis(delay_with_jitter);
            }
        }

        Duration::from_millis(delay_ms)
    }
}

/// Result of a retryable operation.
#[derive(Debug, Clone)]
pub struct RetryResult<T> {
    /// The final result (success or last error).
    pub result: Result<T, String>,
    /// Number of attempts made (1 = first try succeeded).
    pub attempts: u32,
    /// Total time spent retrying.
    pub total_duration: Duration,
    /// Whether the operation ultimately succeeded.
    pub succeeded: bool,
}

impl<T> RetryResult<T> {
    pub fn is_success(&self) -> bool {
        self.succeeded
    }

    pub fn into_result(self) -> Result<T, String> {
        self.result
    }
}

/// Execute a function with retry logic.
///
/// # Example
/// ```
/// use semantic::retry::{RetryConfig, execute_with_retry};
/// use std::time::Duration;
///
/// let config = RetryConfig::default()
///     .with_max_retries(3)
///     .with_base_delay(Duration::from_millis(100));
///
/// let result = execute_with_retry(&config, |attempt| {
///     if attempt == 0 {
///         Err("transient error".to_string())
///     } else {
///         Ok("success")
///     }
/// });
///
/// assert!(result.succeeded);
/// ```
pub fn execute_with_retry<T, F>(config: &RetryConfig, mut operation: F) -> RetryResult<T>
where
    F: FnMut(u32) -> Result<T, String>,
{
    let start = std::time::Instant::now();
    let mut last_error = None;

    for attempt in 0..=config.max_retries {
        match operation(attempt) {
            Ok(value) => {
                return RetryResult {
                    result: Ok(value),
                    attempts: attempt + 1,
                    total_duration: start.elapsed(),
                    succeeded: true,
                };
            }
            Err(e) => {
                last_error = Some(e);

                // Don't sleep after the last attempt
                if attempt < config.max_retries {
                    let delay = config.calculate_delay(attempt + 1);
                    if delay > Duration::from_millis(0) {
                        thread::sleep(delay);
                    }
                }
            }
        }
    }

    RetryResult {
        result: Err(last_error.unwrap_or_else(|| "All retries exhausted".to_string())),
        attempts: config.max_retries + 1,
        total_duration: start.elapsed(),
        succeeded: false,
    }
}

/// Determine if an error is retryable based on common HTTP status codes.
pub fn is_retryable_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();

    // Retry on transient network errors
    if error_lower.contains("timeout")
        || error_lower.contains("connection")
        || error_lower.contains("reset")
        || error_lower.contains("refused")
        || error_lower.contains("dns")
        || error_lower.contains("unreachable")
    {
        return true;
    }

    // Retry on specific HTTP status codes (5xx, 429 Too Many Requests)
    if error_lower.contains("503")  // Service Unavailable
        || error_lower.contains("502")  // Bad Gateway
        || error_lower.contains("504")  // Gateway Timeout
        || error_lower.contains("429")  // Too Many Requests
        || error_lower.contains("500")  // Internal Server Error (sometimes transient)
        || error_lower.contains("524")
    // Cloudflare timeout
    {
        return true;
    }

    // Don't retry on 4xx errors (client errors)
    if error_lower.contains("400")  // Bad Request
        || error_lower.contains("401")  // Unauthorized
        || error_lower.contains("403")  // Forbidden
        || error_lower.contains("404")  // Not Found
        || error_lower.contains("422")
    // Unprocessable Entity
    {
        return false;
    }

    // Default: retry unknown errors (conservative approach)
    true
}

/// Execute an async function with retry logic.
///
/// # Example
/// ```
/// use semantic::retry::{RetryConfig, execute_with_retry_async};
/// use std::time::Duration;
///
/// async fn example() {
///     let config = RetryConfig::default()
///         .with_max_retries(3)
///         .with_base_delay(Duration::from_millis(100));
///
///     let result = execute_with_retry_async(&config, |attempt| async move {
///         if attempt == 0 {
///             Err("transient error".to_string())
///         } else {
///             Ok("success")
///         }
///     }).await;
///
///     assert!(result.succeeded);
/// }
/// ```
pub async fn execute_with_retry_async<T, F, Fut>(
    config: &RetryConfig,
    mut operation: F,
) -> RetryResult<T>
where
    F: FnMut(u32) -> Fut,
    Fut: std::future::Future<Output = Result<T, String>>,
{
    let start = std::time::Instant::now();
    let mut last_error = None;

    for attempt in 0..=config.max_retries {
        match operation(attempt).await {
            Ok(value) => {
                return RetryResult {
                    result: Ok(value),
                    attempts: attempt + 1,
                    total_duration: start.elapsed(),
                    succeeded: true,
                };
            }
            Err(e) => {
                last_error = Some(e);

                // Don't sleep after the last attempt
                if attempt < config.max_retries {
                    let delay = config.calculate_delay(attempt + 1);
                    if delay > Duration::from_millis(0) {
                        sleep(delay).await;
                    }
                }
            }
        }
    }

    RetryResult {
        result: Err(last_error.unwrap_or_else(|| "All retries exhausted".to_string())),
        attempts: config.max_retries + 1,
        total_duration: start.elapsed(),
        succeeded: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay, Duration::from_millis(100));
        assert_eq!(config.max_delay, Duration::from_secs(5));
        assert!(config.jitter);
    }

    #[test]
    fn test_calculate_delay_no_delay_on_first_attempt() {
        let config = RetryConfig::default();
        assert_eq!(config.calculate_delay(0), Duration::from_millis(0));
    }

    #[test]
    fn test_calculate_delay_exponential() {
        let config = RetryConfig::default()
            .with_base_delay(Duration::from_millis(100))
            .with_backoff_multiplier(2.0)
            .with_jitter(false);

        // Attempt 1: 100ms * 2^0 = 100ms
        assert_eq!(config.calculate_delay(1), Duration::from_millis(100));

        // Attempt 2: 100ms * 2^1 = 200ms
        assert_eq!(config.calculate_delay(2), Duration::from_millis(200));

        // Attempt 3: 100ms * 2^2 = 400ms
        assert_eq!(config.calculate_delay(3), Duration::from_millis(400));
    }

    #[test]
    fn test_calculate_delay_respects_max() {
        let config = RetryConfig::default()
            .with_base_delay(Duration::from_secs(1))
            .with_max_delay(Duration::from_millis(500))
            .with_backoff_multiplier(10.0)
            .with_jitter(false);

        // Should be capped at max_delay even if calculation exceeds it
        let delay = config.calculate_delay(1);
        assert!(delay <= Duration::from_millis(500));
    }

    #[test]
    fn test_execute_with_retry_success_first_try() {
        let config = RetryConfig::default();
        let result = execute_with_retry(&config, |_attempt| Ok::<&str, String>("success"));

        assert!(result.succeeded);
        assert_eq!(result.attempts, 1);
        assert_eq!(result.result.unwrap(), "success");
    }

    #[test]
    fn test_execute_with_retry_eventual_success() {
        let config = RetryConfig::default().with_max_retries(3);

        let attempts = std::cell::RefCell::new(0);
        let result = execute_with_retry(&config, |_attempt| {
            let mut count = attempts.borrow_mut();
            *count += 1;
            if *count < 3 {
                Err("transient error".to_string())
            } else {
                Ok("success")
            }
        });

        assert!(result.succeeded);
        assert_eq!(result.attempts, 3);
    }

    #[test]
    fn test_execute_with_retry_exhaustion() {
        let config = RetryConfig::default()
            .with_max_retries(2)
            .with_base_delay(Duration::from_millis(10));

        let result: RetryResult<()> =
            execute_with_retry(&config, |_attempt| Err("persistent error".to_string()));

        assert!(!result.succeeded);
        assert_eq!(result.attempts, 3); // initial + 2 retries
    }

    #[test]
    fn test_is_retryable_timeout() {
        assert!(is_retryable_error("Request timeout"));
        assert!(is_retryable_error("connection timeout"));
    }

    #[test]
    fn test_is_retryable_5xx() {
        assert!(is_retryable_error("HTTP 503"));
        assert!(is_retryable_error("502 Bad Gateway"));
        assert!(is_retryable_error("429 Too Many Requests"));
    }

    #[test]
    fn test_is_not_retryable_4xx() {
        assert!(!is_retryable_error("HTTP 400"));
        assert!(!is_retryable_error("401 Unauthorized"));
        assert!(!is_retryable_error("404 Not Found"));
    }

    #[test]
    fn test_builder_methods() {
        let config = RetryConfig::default()
            .with_max_retries(5)
            .with_base_delay(Duration::from_millis(200))
            .with_max_delay(Duration::from_secs(10))
            .with_backoff_multiplier(3.0)
            .with_jitter(false);

        assert_eq!(config.max_retries, 5);
        assert_eq!(config.base_delay, Duration::from_millis(200));
        assert_eq!(config.max_delay, Duration::from_secs(10));
        assert_eq!(config.backoff_multiplier, 3.0);
        assert!(!config.jitter);
    }
}
