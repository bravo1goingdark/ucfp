use crate::config::ServerConfig;
use crate::error::ServerResult;
use dashmap::DashMap;
use index::{IndexConfig, UfpIndex};
use matcher::DefaultMatcher;
use std::sync::Arc;

/// Shared application state
#[derive(Clone)]
pub struct ServerState {
    /// Server configuration
    pub config: Arc<ServerConfig>,

    /// Rate limit tracking: API key -> (count, window_start)
    pub rate_limiter: Arc<DashMap<String, (u32, std::time::Instant)>>,

    /// Index instance (shared across requests)
    pub index: Arc<UfpIndex>,

    /// Matcher instance (shared across requests)
    pub matcher: Arc<DefaultMatcher>,
}

impl ServerState {
    /// Create new server state
    pub fn new(config: ServerConfig) -> ServerResult<Self> {
        // Initialize index with in-memory backend for now
        // In production, this would use RocksDB
        let index_config = IndexConfig::new().with_backend(index::BackendConfig::in_memory());
        let index = Arc::new(UfpIndex::new(index_config)?);

        // Initialize matcher with shared index and default configs
        let matcher = Arc::new(DefaultMatcher::with_index_arc(
            index.clone(),
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        ));

        Ok(Self {
            config: Arc::new(config),
            rate_limiter: Arc::new(DashMap::new()),
            index,
            matcher,
        })
    }

    /// Check if API key is valid
    pub fn is_valid_api_key(&self, key: &str) -> bool {
        self.config.api_keys.contains(key)
    }

    /// Check rate limit for API key
    pub fn check_rate_limit(&self, key: &str) -> bool {
        let now = std::time::Instant::now();
        let window = std::time::Duration::from_secs(60);
        let limit = self.config.rate_limit_per_minute;

        let mut entry = self.rate_limiter.entry(key.to_string()).or_insert((0, now));
        let (count, window_start) = entry.value_mut();

        // Reset if window has passed
        if now.duration_since(*window_start) > window {
            *count = 0;
            *window_start = now;
        }

        // Check limit
        if *count >= limit {
            return false;
        }

        *count += 1;
        true
    }
}

/// Server metadata for health checks
#[derive(Debug, serde::Serialize)]
pub struct ServerMetadata {
    pub version: String,
    pub uptime_seconds: u64,
}
