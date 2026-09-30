use rstarter::api::routes;
use rstarter::config::{Config, LogFormat};
use rstarter::state::build_state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env if present (local development convenience; ignored in
    // containers where the environment is provided by the orchestrator).
    dotenvy::dotenv().ok();

    // 1-3. Load and validate configuration, fail fast.
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("fatal: invalid configuration: {error:#}");
            std::process::exit(1);
        }
    };

    // 2. Initialize structured logging (RUST_LOG + LOG_FORMAT).
    init_tracing(&config);

    // 4-8. Connect to PostgreSQL and Redis, build repositories, services,
    // and the application state. Fails fast if required infrastructure
    // cannot be initialized.
    let state = build_state(&config).await?;

    // Install the Prometheus recorder before serving requests.
    rstarter::api::metrics::init();

    // 9. Build the Axum router.
    let app = routes::build_router(state, config.cors_allowed_origins.clone());

    // 10. Start the HTTP server with graceful shutdown.
    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "rstarter listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("server stopped");
    Ok(())
}

fn init_tracing(config: &Config) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    match config.log_format {
        LogFormat::Pretty => {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(false)
                .init();
        }
        LogFormat::Json => {
            tracing_subscriber::fmt()
                .json()
                .with_env_filter(filter)
                .init();
        }
    }
}

/// Wait for SIGINT or SIGTERM, then signal the server to drain in-flight
/// requests and stop accepting new ones.
#[cfg(unix)]
async fn shutdown_signal() {
    // Installing a SIGTERM handler cannot fail on a normal Unix process;
    // without it graceful container shutdown would be impossible, so the
    // invariant makes failure impossible.
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("failed to install SIGTERM handler");

    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            if let Err(error) = result {
                tracing::error!(%error, "failed to listen for SIGINT");
            }
            tracing::info!("shutdown signal received (SIGINT)");
        }
        _ = terminate.recv() => {
            tracing::info!("shutdown signal received (SIGTERM)");
        }
    }
}
