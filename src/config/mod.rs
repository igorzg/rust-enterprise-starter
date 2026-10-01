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
        let cors_allowed_origins = parse_cors_origins(&env_or("CORS_ALLOWED_ORIGINS", ""))?;

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

/// Parse and validate `CORS_ALLOWED_ORIGINS`: comma-separated, empty
/// entries ignored, each entry must be an origin a browser can actually
/// send (`*` or an absolute `http(s)://` URI with an authority).
/// Invalid entries fail startup instead of being silently dropped.
pub fn parse_cors_origins(raw: &str) -> Result<Vec<String>> {
    raw.split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| -> Result<String> {
            validate_origin(origin).with_context(|| {
                format!("CORS_ALLOWED_ORIGINS entry is not a valid origin: {origin}")
            })?;
            Ok(origin.to_string())
        })
        .collect()
}

/// An origin is `*` (any origin) or an absolute `http(s)://` URI with an
/// authority and no path/query — anything else can never match a request
/// `Origin` header, so it is a misconfiguration.
fn validate_origin(origin: &str) -> Result<()> {
    if origin == "*" {
        return Ok(());
    }
    let uri = axum::http::Uri::from_str(origin)?;
    let scheme = uri.scheme_str().map(|scheme| scheme.to_ascii_lowercase());
    // An origin has no path; the http crate represents the (required)
    // empty path as `/`.
    let no_path = matches!(
        uri.path_and_query().map(|path| path.as_str()),
        None | Some("/")
    );
    if matches!(scheme.as_deref(), Some("http" | "https")) && uri.authority().is_some() && no_path {
        Ok(())
    } else {
        Err(anyhow::anyhow!("not an absolute http(s) origin"))
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
