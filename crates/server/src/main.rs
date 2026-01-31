//! UCFP Server - HTTP REST API for Universal Content Fingerprinting
//!
//! This binary provides a production-ready HTTP server exposing UCFP
//! functionality via REST endpoints with authentication and rate limiting.

use server::ServerConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration
    let config = ServerConfig::load()?;

    // Start server
    server::start_server(config).await?;

    Ok(())
}
