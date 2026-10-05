use std::sync::Arc;

use anyhow::Result;

use crate::config::Config;
use crate::persistence::cache::redis::{RedisGroupCache, RedisUserCache};
use crate::persistence::database::postgres::{PostgresGroupRepository, PostgresUserRepository};
use crate::persistence::health::SystemHealthCheck;
use crate::services::group_service::GroupService;
use crate::services::ports::{GroupCache, GroupRepository, HealthCheck, UserCache, UserRepository};
use crate::services::user_service::UserService;

/// Shared application state, passed to handlers via Axum `State`.
///
/// This is the composition root: traits are wired to their concrete
/// persistence implementations here (and in tests, to in-memory
/// fakes).
#[derive(Clone)]
pub struct AppState {
    pub user_service: Arc<UserService>,
    pub group_service: Arc<GroupService>,
    pub health: Arc<dyn HealthCheck>,
}

/// Build the application state: connect to PostgreSQL and Redis, then
/// assemble repositories, services, and health probes.
pub async fn build_state(config: &Config) -> Result<AppState> {
    let pool = crate::persistence::database::create_pool(&config.database_url).await?;

    let user_cache: Arc<dyn UserCache> = Arc::new(
        RedisUserCache::connect(&config.redis_url, config.cache_ttl)
            .await
            .map_err(|e| anyhow::anyhow!("Redis: {e}"))?,
    );

    let group_cache: Arc<dyn GroupCache> = Arc::new(
        RedisGroupCache::connect(&config.redis_url, config.cache_ttl)
            .await
            .map_err(|e| anyhow::anyhow!("Redis: {e}"))?,
    );

    let user_repository: Arc<dyn UserRepository> =
        Arc::new(PostgresUserRepository::new(pool.clone()));

    let group_repository: Arc<dyn GroupRepository> =
        Arc::new(PostgresGroupRepository::new(pool.clone()));

    let user_service = Arc::new(UserService::new(
        user_repository.clone(),
        user_cache.clone(),
    ));

    let group_service = Arc::new(GroupService::new(
        group_repository,
        user_repository,
        group_cache,
    ));

    let health: Arc<dyn HealthCheck> = Arc::new(SystemHealthCheck::new(pool, user_cache));

    Ok(AppState {
        user_service,
        group_service,
        health,
    })
}
