use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::time::Duration;

/// Server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Server bind address
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,

    /// Server port
    #[serde(default = "default_port")]
    pub port: u16,

    /// Request timeout in seconds
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// Maximum request body size in MB
    #[serde(default = "default_max_body_size_mb")]
    pub max_body_size_mb: usize,

    /// Rate limit: requests per minute per API key
    #[serde(default = "default_rate_limit_per_minute")]
    pub rate_limit_per_minute: u32,

    /// API keys for authentication (in production, use a database)
    #[serde(default)]
    pub api_keys: HashSet<String>,

    /// Enable CORS
    #[serde(default = "default_true")]
    pub enable_cors: bool,

    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Metrics endpoint enabled
    #[serde(default = "default_true")]
    pub metrics_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: default_bind_addr(),
            port: default_port(),
            timeout_secs: default_timeout_secs(),
            max_body_size_mb: default_max_body_size_mb(),
            rate_limit_per_minute: default_rate_limit_per_minute(),
            api_keys: HashSet::new(),
            enable_cors: default_true(),
            log_level: default_log_level(),
            metrics_enabled: default_true(),
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables and config files
    pub fn load() -> anyhow::Result<Self> {
        let builder = config::Config::builder()
            // Load from file if exists
            .add_source(config::File::with_name("server").required(false))
            // Override with environment variables
            .add_source(config::Environment::with_prefix("UCFP_SERVER").separator("__"));

        let config: ServerConfig = builder.build()?.try_deserialize()?;

        // Add demo API key if none configured (for development)
        let mut config = config;
        if config.api_keys.is_empty() {
            tracing::warn!("No API keys configured, using demo key 'demo-key-12345'");
            config.api_keys.insert("demo-key-12345".to_string());
        }

        Ok(config)
    }

    /// Get the socket address to bind to
    pub fn socket_addr(&self) -> anyhow::Result<SocketAddr> {
        let addr_str = format!("{}:{}", self.bind_addr, self.port);
        Ok(addr_str.parse()?)
    }

    /// Get request timeout as Duration
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_secs)
    }

    /// Get max body size in bytes
    pub fn max_body_size(&self) -> usize {
        self.max_body_size_mb * 1024 * 1024
    }
}

fn default_bind_addr() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_timeout_secs() -> u64 {
    30
}

fn default_max_body_size_mb() -> usize {
    10
}

fn default_rate_limit_per_minute() -> u32 {
    100
}

fn default_true() -> bool {
    true
}

fn default_log_level() -> String {
    "info".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = ServerConfig::default();
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.timeout_secs, 30);
        assert_eq!(cfg.max_body_size_mb, 10);
        assert_eq!(cfg.rate_limit_per_minute, 100);
        assert!(cfg.enable_cors);
        assert!(cfg.metrics_enabled);
    }

    #[test]
    fn test_socket_addr() {
        let cfg = ServerConfig::default();
        let addr = cfg.socket_addr().unwrap();
        assert_eq!(addr.port(), 8080);
    }
}
