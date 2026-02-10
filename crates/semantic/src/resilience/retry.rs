//! Retry logic with exponential backoff for transient failures.
//!
//! Automatically retries failed operations with increasing delays to handle
//! temporary issues like network hiccups or rate limiting.

use std::future::Future;
use std::time::Duration;

/// Configuration for retry behavior.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts.
    pub max_retries: u32,
    /// Base delay between retries (exponentially increased).
    pub base_delay: Duration,
    /// Maximum delay between retries.
    pub max_delay: Duration,
    /// Whether to add random jitter to delays.
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Create a new config with custom max retries.
    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    /// Create a new config with custom base delay.
    pub fn with_base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }

    /// Create a new config with custom max delay.
    pub fn with_max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Enable or disable jitter.
    pub fn with_jitter(mut self, jitter: bool) -> Self {
        self.jitter = jitter;
        self
    }
}

/// Result of a retry operation.
#[derive(Debug, Clone)]
pub struct RetryResult<T> {
    /// The final result (Ok if succeeded, Err if all retries failed).
    pub result: Result<T, String>,
    /// Number of attempts made (1 = no retries needed).
    pub attempts: u32,
    /// Total duration spent on all attempts.
    pub total_duration: Duration,
    /// Whether the operation ultimately succeeded.
    pub succeeded: bool,
}

impl<T> RetryResult<T> {
    /// Convert to a standard Result.
    pub fn into_result(self) -> Result<T, String> {
        self.result
    }
}

/// Execute an operation with retry logic.
///
/// # Example
///
/// ```ignore
/// use semantic::resilience::{RetryConfig, execute_with_retry};
///
/// let config = RetryConfig::default();
/// let result = execute_with_retry(&config, |attempt| {
///     if attempt < 2 {
///         Err("transient error".to_string())
///     } else {
///         Ok("success")
///     }
/// });
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
            Err(error) => {
                last_error = Some(error);

                if attempt < config.max_retries {
                    let delay = calculate_delay(config, attempt);
                    std::thread::sleep(delay);
                }
            }
        }
    }

    RetryResult {
        result: Err(last_error.unwrap_or_else(|| "All retries failed".to_string())),
        attempts: config.max_retries + 1,
        total_duration: start.elapsed(),
        succeeded: false,
    }
}

/// Execute an async operation with retry logic.
///
/// # Example
///
/// ```ignore
/// use semantic::resilience::{RetryConfig, execute_with_retry_async};
///
/// async fn example() {
///     let config = RetryConfig::default();
///     let result = execute_with_retry_async(&config, |attempt| async move {
///         // Your async operation here
///         Ok::<_, String>("success")
///     }).await;
/// }
/// ```
pub async fn execute_with_retry_async<T, F, Fut>(
    config: &RetryConfig,
    mut operation: F,
) -> RetryResult<T>
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<T, String>>,
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
            Err(error) => {
                last_error = Some(error);

                if attempt < config.max_retries {
                    let delay = calculate_delay(config, attempt);
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    RetryResult {
        result: Err(last_error.unwrap_or_else(|| "All retries failed".to_string())),
        attempts: config.max_retries + 1,
        total_duration: start.elapsed(),
        succeeded: false,
    }
}

/// Calculate delay for a retry attempt with exponential backoff.
fn calculate_delay(config: &RetryConfig, attempt: u32) -> Duration {
    let base = config.base_delay.as_millis() as u64;
    let exponential = base * 2_u64.pow(attempt);
    let delay = exponential.min(config.max_delay.as_millis() as u64);

    if config.jitter {
        // Add 0-50% random jitter
        let jitter = fastrand::u64(0..=delay / 2);
        Duration::from_millis(delay + jitter)
    } else {
        Duration::from_millis(delay)
    }
}

/// Check if an error is retryable (transient).
///
/// Non-retryable errors return immediately without wasting retries.
pub fn is_retryable_error(error: &str) -> bool {
    let error_lower = error.to_lowercase();

    // Retryable errors
    if error_lower.contains("timeout")
        || error_lower.contains("connection")
        || error_lower.contains("reset")
        || error_lower.contains("temporarily")
        || error_lower.contains("unavailable")
        || error_lower.contains("503") // Service Unavailable
        || error_lower.contains("502") // Bad Gateway
        || error_lower.contains("429") // Too Many Requests
        || error_lower.contains("504") // Gateway Timeout
        || error_lower.contains("408")
    // Request Timeout
    {
        return true;
    }

    // Non-retryable errors
    if error_lower.contains("401") // Unauthorized
        || error_lower.contains("403") // Forbidden
        || error_lower.contains("404") // Not Found
        || error_lower.contains("400") // Bad Request
        || error_lower.contains("invalid")
        || error_lower.contains("not found")
    {
        return false;
    }

    // Default to retryable for unknown errors
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_succeeds_eventually() {
        let config = RetryConfig::default().with_max_retries(3);
        let mut counter = 0;

        let result = execute_with_retry(&config, |_attempt| {
            counter += 1;
            if counter < 3 {
                Err("not yet".to_string())
            } else {
                Ok("success")
            }
        });

        assert!(result.succeeded);
        assert_eq!(result.attempts, 3);
        assert_eq!(result.into_result().unwrap(), "success");
    }

    #[test]
    fn retry_fails_after_max_attempts() {
        let config = RetryConfig::default().with_max_retries(2);

        let result: RetryResult<String> =
            execute_with_retry(&config, |_attempt| Err("always fails".to_string()));

        assert!(!result.succeeded);
        assert_eq!(result.attempts, 3); // Initial + 2 retries
        assert!(result.into_result().is_err());
    }

    #[test]
    fn retry_no_delay_on_success() {
        let config = RetryConfig::default();

        let result = execute_with_retry(&config, |_attempt| Ok("immediate success"));

        assert!(result.succeeded);
        assert_eq!(result.attempts, 1);
        assert!(result.total_duration < Duration::from_millis(10));
    }

    #[test]
    fn is_retryable_error_detection() {
        assert!(is_retryable_error("timeout"));
        assert!(is_retryable_error("connection reset"));
        assert!(is_retryable_error("HTTP 503"));
        assert!(is_retryable_error("HTTP 429"));
        assert!(is_retryable_error("service temporarily unavailable"));

        assert!(!is_retryable_error("HTTP 400"));
        assert!(!is_retryable_error("HTTP 401"));
        assert!(!is_retryable_error("HTTP 404"));
        assert!(!is_retryable_error("invalid api key"));
    }
}
