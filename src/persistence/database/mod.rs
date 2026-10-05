use anyhow::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};

pub mod domain;
pub mod postgres;

/// Open a connection pool to PostgreSQL and verify connectivity.
///
/// Migrations are intentionally NOT run here; the dedicated `migrate`
/// service owns schema changes.
pub async fn create_pool(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await
        .context("failed to connect to PostgreSQL")?;

    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .context("database connectivity check failed")?;

    Ok(pool)
}
