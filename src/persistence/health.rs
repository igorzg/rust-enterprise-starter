use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use tracing::warn;

use crate::services::ports::{ComponentStatus, HealthCheck, HealthStatus, UserCache};

/// Readiness probe that checks PostgreSQL and Redis directly.
pub struct SystemHealthCheck {
    pool: PgPool,
    cache: Arc<dyn UserCache>,
}

impl SystemHealthCheck {
    pub fn new(pool: PgPool, cache: Arc<dyn UserCache>) -> Self {
        Self { pool, cache }
    }
}

#[async_trait]
impl HealthCheck for SystemHealthCheck {
    async fn check(&self) -> HealthStatus {
        let postgres = match sqlx::query("SELECT 1").fetch_one(&self.pool).await {
            Ok(_) => true,
            Err(error) => {
                warn!(%error, "readiness check: PostgreSQL is down");
                false
            }
        };
        let redis = match self.cache.ping().await {
            Ok(()) => true,
            Err(error) => {
                warn!(%error, "readiness check: Redis is down");
                false
            }
        };
        HealthStatus {
            components: vec![
                ComponentStatus {
                    name: "postgres",
                    up: postgres,
                },
                ComponentStatus {
                    name: "redis",
                    up: redis,
                },
            ],
        }
    }
}
