use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result};

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    /// Human-readable single-line logs (default, development-friendly).
    Pretty,
    /// One JSON object per line (structured, for log aggregators).
    Json,
}

impl LogFormat {
    fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "pretty" | "text" | "human" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            other => Err(anyhow::anyhow!(
                "LOG_FORMAT must be \"pretty\" or \"json\", got \"{other}\""
            )),
        }
    }
}

/// Centralized application configuration, loaded from environment variables.
///
/// The application fails fast at startup when required configuration is
/// missing or invalid.
#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub redis_url: String,
    pub cors_allowed_origins: Vec<String>,
    pub cache_ttl: Duration,
    pub log_format: LogFormat,
}

impl Config {
    /// Read and validate configuration from the environment.
    pub fn from_env() -> Result<Self> {
        let host = env_or("APP_HOST", "0.0.0.0");
        let port = env_port("APP_PORT", 8080)?;
        let bind_addr = SocketAddr::from_str(&format!("{host}:{port}"))
            .with_context(|| "APP_HOST:APP_PORT is not a valid socket address")?;
        let database_url = env_required("DATABASE_URL")?;
        let redis_url = env_required("REDIS_URL")?;
        let log_format = LogFormat::parse(&env_or("LOG_FORMAT", "pretty"))?;
        let cache_ttl = Duration::from_secs(
            env_or("CACHE_TTL_SECS", "300")
                .parse::<u64>()
                .context("CACHE_TTL_SECS must be a non-negative integer")?,
        );
        let cors_allowed_origins = env_or("CORS_ALLOWED_ORIGINS", "")
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_string)
            .collect();

        Ok(Self {
            bind_addr,
            database_url,
            redis_url,
            cors_allowed_origins,
            cache_ttl,
            log_format,
        })
    }
}

fn env_or(key: &str, default: &str) -> String {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => default.to_string(),
    }
}

fn env_port(key: &str, default: u16) -> Result<u16> {
    env_or(key, &default.to_string())
        .parse()
        .with_context(|| "{key} must be a valid port number")
}

fn env_required(key: &str) -> Result<String> {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        _ => Err(anyhow::anyhow!(
            "missing required environment variable: {key}"
        )),
    }
}
