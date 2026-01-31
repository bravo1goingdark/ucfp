//! Rate limiting for API providers.
//!
//! Prevents exceeding provider rate limits using token bucket algorithm.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// Configuration for rate limiting.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct RateLimitConfig {
    /// Maximum requests per second (sustained rate).
    pub requests_per_second: f64,
    /// Burst capacity (maximum requests that can be made instantly).
    pub burst_size: u64,
    /// Maximum wait time for a token in milliseconds (0 = fail immediately if no token available).
    #[serde(with = "crate::serde_millis")]
    pub max_wait: Duration,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 10.0,
            burst_size: 20,
            max_wait: Duration::from_secs(5),
        }
    }
}

impl RateLimitConfig {
    pub fn with_requests_per_second(mut self, rps: f64) -> Self {
        self.requests_per_second = rps;
        self
    }

    pub fn with_burst_size(mut self, burst: u64) -> Self {
        self.burst_size = burst;
        self
    }

    pub fn with_max_wait(mut self, wait: Duration) -> Self {
        self.max_wait = wait;
        self
    }
}

/// Token bucket rate limiter.
#[derive(Debug)]
pub struct TokenBucket {
    config: RateLimitConfig,
    tokens: Mutex<f64>,
    last_update: Mutex<Instant>,
    total_requests: AtomicU64,
    total_waited: AtomicU64,
    total_rejected: AtomicU64,
}

impl TokenBucket {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            tokens: Mutex::new(config.burst_size as f64),
            last_update: Mutex::new(Instant::now()),
            total_requests: AtomicU64::new(0),
            total_waited: AtomicU64::new(0),
            total_rejected: AtomicU64::new(0),
        }
    }

    /// Add tokens based on elapsed time.
    fn add_tokens(&self) {
        let mut last_update = self.last_update.lock().unwrap();
        let mut tokens = self.tokens.lock().unwrap();

        let now = Instant::now();
        let elapsed = now.duration_since(*last_update).as_secs_f64();
        *last_update = now;

        // Add tokens based on rate: tokens = elapsed * requests_per_second
        let new_tokens = elapsed * self.config.requests_per_second;
        *tokens = (*tokens + new_tokens).min(self.config.burst_size as f64);
    }

    /// Try to acquire a token without waiting.
    pub fn try_acquire(&self) -> bool {
        self.add_tokens();

        let mut tokens = self.tokens.lock().unwrap();
        self.total_requests.fetch_add(1, Ordering::SeqCst);

        if *tokens >= 1.0 {
            *tokens -= 1.0;
            true
        } else {
            self.total_rejected.fetch_add(1, Ordering::SeqCst);
            false
        }
    }

    /// Acquire a token, waiting up to max_wait if necessary.
    /// Returns true if token acquired, false if timeout.
    pub fn acquire(&self) -> bool {
        self.total_requests.fetch_add(1, Ordering::SeqCst);

        let start = Instant::now();

        loop {
            self.add_tokens();

            let mut tokens = self.tokens.lock().unwrap();

            if *tokens >= 1.0 {
                *tokens -= 1.0;
                let waited = start.elapsed();
                if waited > Duration::from_millis(0) {
                    self.total_waited.fetch_add(1, Ordering::SeqCst);
                }
                return true;
            }

            // Check if we've exceeded max wait time
            if start.elapsed() >= self.config.max_wait {
                self.total_rejected.fetch_add(1, Ordering::SeqCst);
                return false;
            }

            // Calculate how long to wait for next token
            let tokens_needed = 1.0 - *tokens;
            let wait_seconds = tokens_needed / self.config.requests_per_second;
            let wait_duration = Duration::from_secs_f64(wait_seconds.min(0.1));

            // Drop lock before sleeping
            drop(tokens);

            thread::sleep(wait_duration);
        }
    }

    /// Get current statistics.
    pub fn stats(&self) -> RateLimitStats {
        RateLimitStats {
            available_tokens: *self.tokens.lock().unwrap(),
            total_requests: self.total_requests.load(Ordering::SeqCst),
            total_waited: self.total_waited.load(Ordering::SeqCst),
            total_rejected: self.total_rejected.load(Ordering::SeqCst),
        }
    }
}

/// Statistics for rate limiter.
#[derive(Debug, Clone, Copy)]
pub struct RateLimitStats {
    pub available_tokens: f64,
    pub total_requests: u64,
    pub total_waited: u64,
    pub total_rejected: u64,
}

impl RateLimitStats {
    /// Calculate rejection rate (0.0 to 1.0).
    pub fn rejection_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_rejected as f64 / self.total_requests as f64
        }
    }

    /// Calculate wait rate (0.0 to 1.0).
    pub fn wait_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.total_waited as f64 / self.total_requests as f64
        }
    }
}

/// Manager for multiple rate limiters (one per provider).
#[derive(Debug)]
pub struct RateLimitManager {
    buckets: dashmap::DashMap<String, Arc<TokenBucket>>,
    default_config: RateLimitConfig,
}

impl RateLimitManager {
    pub fn new(default_config: RateLimitConfig) -> Self {
        Self {
            buckets: dashmap::DashMap::new(),
            default_config,
        }
    }

    /// Get or create rate limiter for a provider.
    pub fn get_or_create(&self, provider: &str) -> Arc<TokenBucket> {
        self.buckets
            .entry(provider.to_string())
            .or_insert_with(|| Arc::new(TokenBucket::new(self.default_config)))
            .clone()
    }

    /// Get rate limiter for provider with custom config.
    pub fn get_or_create_with_config(
        &self,
        provider: &str,
        config: RateLimitConfig,
    ) -> Arc<TokenBucket> {
        self.buckets
            .entry(provider.to_string())
            .or_insert_with(|| Arc::new(TokenBucket::new(config)))
            .clone()
    }

    /// Get stats for all providers.
    pub fn get_all_stats(&self) -> Vec<(String, RateLimitStats)> {
        self.buckets
            .iter()
            .map(|entry| {
                let (name, bucket) = entry.pair();
                (name.clone(), bucket.stats())
            })
            .collect()
    }

    /// Reset all rate limiters (useful for testing or config changes).
    pub fn reset_all(&self) {
        self.buckets.clear();
    }
}

impl Default for RateLimitManager {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

/// Common rate limit configs for popular providers.
pub mod presets {
    use super::*;

    /// OpenAI typical rate limits (moderate tier).
    pub fn openai() -> RateLimitConfig {
        RateLimitConfig {
            requests_per_second: 3.0, // ~180 RPM
            burst_size: 10,
            max_wait: Duration::from_secs(30),
        }
    }

    /// HuggingFace Inference API (free tier).
    pub fn huggingface_free() -> RateLimitConfig {
        RateLimitConfig {
            requests_per_second: 1.0, // Very conservative
            burst_size: 3,
            max_wait: Duration::from_secs(10),
        }
    }

    /// HuggingFace Inference API (pro tier).
    pub fn huggingface_pro() -> RateLimitConfig {
        RateLimitConfig {
            requests_per_second: 10.0,
            burst_size: 30,
            max_wait: Duration::from_secs(30),
        }
    }

    /// Aggressive local/self-hosted API.
    pub fn local_api() -> RateLimitConfig {
        RateLimitConfig {
            requests_per_second: 100.0,
            burst_size: 200,
            max_wait: Duration::from_secs(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_bucket_starts_full() {
        let config = RateLimitConfig::default().with_burst_size(10);
        let bucket = TokenBucket::new(config);

        let stats = bucket.stats();
        assert_eq!(stats.available_tokens, 10.0);
    }

    #[test]
    fn test_token_bucket_acquires() {
        let config = RateLimitConfig::default().with_burst_size(5);
        let bucket = TokenBucket::new(config);

        // Should be able to acquire burst_size tokens immediately
        for _ in 0..5 {
            assert!(bucket.try_acquire());
        }

        // Should fail now (empty)
        assert!(!bucket.try_acquire());

        let stats = bucket.stats();
        assert_eq!(stats.total_requests, 6);
        assert_eq!(stats.total_rejected, 1);
    }

    #[test]
    fn test_token_bucket_refills() {
        let config = RateLimitConfig::default()
            .with_requests_per_second(100.0) // 100 RPS = 1 token per 10ms
            .with_burst_size(1);
        let bucket = TokenBucket::new(config);

        // Empty the bucket
        assert!(bucket.try_acquire());
        assert!(!bucket.try_acquire());

        // Wait for refill
        thread::sleep(Duration::from_millis(20));

        // Should have refilled
        assert!(bucket.try_acquire());
    }

    #[test]
    fn test_acquire_with_wait() {
        let config = RateLimitConfig::default()
            .with_requests_per_second(100.0) // 1 token per 10ms
            .with_burst_size(1)
            .with_max_wait(Duration::from_millis(100));
        let bucket = TokenBucket::new(config);

        // Empty the bucket
        assert!(bucket.try_acquire());

        // Try to acquire (should wait for refill)
        let acquired = bucket.acquire();
        assert!(acquired);

        let stats = bucket.stats();
        assert_eq!(stats.total_waited, 1);
    }

    #[test]
    fn test_acquire_timeout() {
        let config = RateLimitConfig::default()
            .with_requests_per_second(0.1) // 1 token per 10 seconds
            .with_burst_size(1)
            .with_max_wait(Duration::from_millis(50)); // Very short wait
        let bucket = TokenBucket::new(config);

        // Empty the bucket
        assert!(bucket.try_acquire());

        // Try to acquire (should timeout)
        let acquired = bucket.acquire();
        assert!(!acquired);

        let stats = bucket.stats();
        assert_eq!(stats.total_rejected, 1);
    }

    #[test]
    fn test_manager_creates_buckets() {
        let manager = RateLimitManager::default();
        let bucket = manager.get_or_create("openai");

        assert!(bucket.try_acquire());

        let stats = manager.get_all_stats();
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].0, "openai");
    }

    #[test]
    fn test_manager_shares_buckets() {
        let manager = RateLimitManager::default();

        // Get bucket twice
        let bucket1 = manager.get_or_create("provider");
        let bucket2 = manager.get_or_create("provider");

        // Use first bucket
        assert!(bucket1.try_acquire());

        // Second bucket should see the same state
        let stats = bucket2.stats();
        assert_eq!(stats.total_requests, 1);
    }

    #[test]
    fn test_preset_configs() {
        let openai = presets::openai();
        assert_eq!(openai.requests_per_second, 3.0);

        let hf_free = presets::huggingface_free();
        assert_eq!(hf_free.requests_per_second, 1.0);

        let local = presets::local_api();
        assert_eq!(local.requests_per_second, 100.0);
    }

    #[test]
    fn test_stats_rejection_rate() {
        let stats = RateLimitStats {
            available_tokens: 0.0,
            total_requests: 100,
            total_waited: 0,
            total_rejected: 25,
        };

        assert_eq!(stats.rejection_rate(), 0.25);
    }
}
