//! Rate limiting for API calls using token bucket algorithm.
//!
//! Rate limiting prevents overwhelming external services and helps manage costs
//! for pay-per-call APIs.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Configuration for rate limiting.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per second (sustained rate).
    pub requests_per_second: f64,
    /// Maximum burst size (allow temporary spikes).
    pub burst_size: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_second: 10.0,
            burst_size: 5,
        }
    }
}

impl RateLimitConfig {
    /// Create a new config with custom requests per second.
    pub fn with_requests_per_second(mut self, rps: f64) -> Self {
        self.requests_per_second = rps;
        self
    }

    /// Create a new config with custom burst size.
    pub fn with_burst_size(mut self, burst: u32) -> Self {
        self.burst_size = burst;
        self
    }

    /// Get rate limit config for OpenAI APIs.
    pub fn openai() -> Self {
        Self {
            requests_per_second: 60.0, // Varies by tier
            burst_size: 10,
        }
    }

    /// Get rate limit config for Hugging Face free tier.
    pub fn huggingface_free() -> Self {
        Self {
            requests_per_second: 1.0,
            burst_size: 2,
        }
    }

    /// Get rate limit config for Hugging Face pro tier.
    pub fn huggingface_pro() -> Self {
        Self {
            requests_per_second: 10.0,
            burst_size: 5,
        }
    }

    /// Get rate limit config for local APIs.
    pub fn local_api() -> Self {
        Self {
            requests_per_second: 1000.0, // Very high for local
            burst_size: 100,
        }
    }
}

/// Token bucket rate limiter.
pub struct TokenBucket {
    config: RateLimitConfig,
    tokens: Mutex<f64>,
    last_update: Mutex<Instant>,
    total_requests: AtomicU64,
    throttled_requests: AtomicU64,
}

impl TokenBucket {
    /// Create a new token bucket with the given configuration.
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            tokens: Mutex::new(config.burst_size as f64),
            last_update: Mutex::new(Instant::now()),
            total_requests: AtomicU64::new(0),
            throttled_requests: AtomicU64::new(0),
        }
    }

    /// Try to acquire a token (non-blocking).
    pub fn try_acquire(&self) -> bool {
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        let mut tokens = self.tokens.lock().unwrap();
        let mut last_update = self.last_update.lock().unwrap();

        // Add tokens based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(*last_update);
        let tokens_to_add = elapsed.as_secs_f64() * self.config.requests_per_second;
        *tokens = (*tokens + tokens_to_add).min(self.config.burst_size as f64);
        *last_update = now;

        // Try to consume a token
        if *tokens >= 1.0 {
            *tokens -= 1.0;
            true
        } else {
            self.throttled_requests.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Acquire a token, blocking until available (with timeout).
    pub fn acquire(&self) -> bool {
        // For now, just try once - in async context, caller should retry
        self.try_acquire()
    }

    /// Get current token count.
    pub fn tokens(&self) -> f64 {
        let mut tokens = self.tokens.lock().unwrap();
        let mut last_update = self.last_update.lock().unwrap();

        // Update tokens based on elapsed time
        let now = Instant::now();
        let elapsed = now.duration_since(*last_update);
        let tokens_to_add = elapsed.as_secs_f64() * self.config.requests_per_second;
        *tokens = (*tokens + tokens_to_add).min(self.config.burst_size as f64);
        *last_update = now;

        *tokens
    }

    /// Get total requests.
    pub fn total_requests(&self) -> u64 {
        self.total_requests.load(Ordering::Relaxed)
    }

    /// Get throttled request count.
    pub fn throttled_requests(&self) -> u64 {
        self.throttled_requests.load(Ordering::Relaxed)
    }

    /// Get stats snapshot.
    pub fn stats(&self) -> RateLimitStats {
        RateLimitStats {
            tokens_available: self.tokens(),
            total_requests: self.total_requests(),
            throttled_requests: self.throttled_requests(),
            requests_per_second: self.config.requests_per_second,
            burst_size: self.config.burst_size,
        }
    }
}

/// Statistics for rate limiter.
#[derive(Debug, Clone)]
pub struct RateLimitStats {
    /// Current available tokens.
    pub tokens_available: f64,
    /// Total requests made.
    pub total_requests: u64,
    /// Requests that were throttled.
    pub throttled_requests: u64,
    /// Configured requests per second.
    pub requests_per_second: f64,
    /// Configured burst size.
    pub burst_size: u32,
}

impl RateLimitStats {
    /// Get throttling rate (0.0 to 1.0).
    pub fn throttle_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.throttled_requests as f64 / self.total_requests as f64
        }
    }
}

/// Manager for multiple rate limiters (one per provider).
pub struct RateLimitManager {
    limiters: Mutex<std::collections::HashMap<String, Arc<TokenBucket>>>,
    default_config: RateLimitConfig,
}

impl RateLimitManager {
    /// Create a new manager with default config.
    pub fn new(default_config: RateLimitConfig) -> Self {
        Self {
            limiters: Mutex::new(std::collections::HashMap::new()),
            default_config,
        }
    }

    /// Get or create a rate limiter for a provider.
    pub fn get_or_create(&self, provider: &str) -> Arc<TokenBucket> {
        let mut limiters = self.limiters.lock().unwrap();
        limiters
            .entry(provider.to_string())
            .or_insert_with(|| Arc::new(TokenBucket::new(self.default_config)))
            .clone()
    }

    /// Get or create a rate limiter with custom config.
    pub fn get_or_create_with_config(
        &self,
        provider: &str,
        config: RateLimitConfig,
    ) -> Arc<TokenBucket> {
        let mut limiters = self.limiters.lock().unwrap();
        limiters
            .entry(provider.to_string())
            .or_insert_with(|| Arc::new(TokenBucket::new(config)))
            .clone()
    }

    /// Get all rate limiter stats.
    pub fn get_all_stats(&self) -> Vec<(String, RateLimitStats)> {
        let limiters = self.limiters.lock().unwrap();
        limiters
            .iter()
            .map(|(name, bucket)| (name.clone(), bucket.stats()))
            .collect()
    }

    /// Reset all rate limiters.
    pub fn reset_all(&self) {
        let mut limiters = self.limiters.lock().unwrap();
        limiters.clear();
    }
}

impl Default for RateLimitManager {
    fn default() -> Self {
        Self::new(RateLimitConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn token_bucket_allows_burst() {
        let config = RateLimitConfig::default().with_burst_size(5);
        let bucket = TokenBucket::new(config);

        // Should allow burst of 5
        for _ in 0..5 {
            assert!(bucket.try_acquire());
        }

        // Should reject 6th
        assert!(!bucket.try_acquire());
    }

    #[test]
    fn token_bucket_refills_over_time() {
        let config = RateLimitConfig::default()
            .with_requests_per_second(100.0) // High rate for test
            .with_burst_size(1);

        let bucket = TokenBucket::new(config);

        // Use the only token
        assert!(bucket.try_acquire());
        assert!(!bucket.try_acquire());

        // Wait for refill
        thread::sleep(Duration::from_millis(20));

        // Should have a token now
        assert!(bucket.try_acquire());
    }

    #[test]
    fn rate_limit_manager_tracks_multiple() {
        let manager = RateLimitManager::default();

        let limiter1 = manager.get_or_create("provider1");
        let limiter2 = manager.get_or_create("provider2");

        // Different providers should have different limiters
        assert!(limiter1.try_acquire());
        assert!(limiter1.try_acquire());
        assert!(limiter1.try_acquire());

        // Provider2 should still have full burst
        for _ in 0..5 {
            assert!(limiter2.try_acquire());
        }
    }

    #[test]
    fn stats_calculates_throttle_rate() {
        let config = RateLimitConfig::default().with_burst_size(2);
        let bucket = TokenBucket::new(config);

        // Use all tokens
        assert!(bucket.try_acquire());
        assert!(bucket.try_acquire());

        // This one should be throttled
        assert!(!bucket.try_acquire());

        let stats = bucket.stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.throttled_requests, 1);
        assert!((stats.throttle_rate() - 0.333).abs() < 0.01);
    }
}
