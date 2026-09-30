use std::sync::Arc;

use anyhow::Result;

use crate::config::Config;
use crate::persistence::cache::redis::RedisUserCache;
use crate::persistence::database::postgres::PostgresUserRepository;
use crate::persistence::health::SystemHealthCheck;
use crate::services::ports::{HealthCheck, UserCache, UserRepository};
use crate::services::user_service::UserService;

/// Shared application state, passed to handlers via Axum `State`.
///
/// This is the composition root: traits are wired to their concrete
/// persistence implementations here (and in tests, to in-memory
/// fakes).
#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub health: Arc<dyn HealthCheck>,
}

/// Build the application state: connect to PostgreSQL and Redis, then
/// assemble repositories, services, and health probes.
pub async fn build_state(config: &Config) -> Result<AppState> {
    let pool = crate::persistence::database::create_pool(&config.database_url).await?;

    let cache: Arc<dyn UserCache> = Arc::new(
        RedisUserCache::connect(&config.redis_url, config.cache_ttl)
            .await
            .map_err(|e| anyhow::anyhow!("Redis: {e}"))?,
    );

    let repository: Arc<dyn UserRepository> = Arc::new(PostgresUserRepository::new(pool.clone()));

    let user_service = Arc::new(UserService::new(repository, cache.clone()));

    let health: Arc<dyn HealthCheck> = Arc::new(SystemHealthCheck::new(pool, cache));

    Ok(AppState {
        user_service,
        health,
    })
}
