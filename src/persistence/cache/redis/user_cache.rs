use std::time::{Duration, Instant};

use async_trait::async_trait;
use redis::AsyncCommands;
use uuid::Uuid;

use crate::persistence::cache::domain::CachedUser;
use crate::services::domain::User;
use crate::services::errors::CacheError;
use crate::services::ports::UserCache;

/// Redis implementation of the core's user cache port.
///
/// Redis types never leave this module: values are JSON-serialized
/// [`CachedUser`] objects keyed by `user:{id}` with a TTL.
pub struct RedisUserCache {
    conn: redis::aio::MultiplexedConnection,
    ttl: Duration,
}

impl RedisUserCache {
    /// Open a multiplexed connection and verify the server responds. The
    /// handle is shared across all operations: cloning it is cheap and
    /// does not open a new socket.
    pub async fn connect(url: &str, ttl: Duration) -> Result<Self, CacheError> {
        let client = redis::Client::open(url).map_err(|e| CacheError::Backend(e.to_string()))?;
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| CacheError::Backend(e.to_string()))?;

        let _: String = redis::cmd("PING")
            .query_async(&mut conn.clone())
            .await
            .map_err(|e| CacheError::Backend(e.to_string()))?;

        Ok(Self { conn, ttl })
    }

    fn key(id: Uuid) -> String {
        format!("user:{id}")
    }
}

fn record_cache(operation: &'static str, started: Instant, ok: bool) {
    metrics::counter!(
        "cache_operations_total",
        "operation" => operation,
        "result" => if ok { "ok" } else { "error" },
    )
    .increment(1);
    metrics::histogram!("cache_operation_duration_seconds", "operation" => operation)
        .record(started.elapsed().as_secs_f64());
}

#[async_trait]
impl UserCache for RedisUserCache {
    async fn get(&self, id: Uuid) -> Result<Option<User>, CacheError> {
        let key = Self::key(id);
        let mut connection = self.conn.clone();

        let started = Instant::now();
        let raw: Result<Option<String>, CacheError> = connection
            .get(&key)
            .await
            .map_err(|e| CacheError::Backend(format!("cache GET user:{id} failed: {e}")));

        let result = raw.and_then(|raw| match raw {
            None => Ok(None),
            Some(raw) => serde_json::from_str::<CachedUser>(&raw)
                .map(|cached| Some(User::from(cached)))
                .map_err(|e| {
                    CacheError::Payload(format!("cached user:{id} is not valid JSON: {e}"))
                }),
        });

        match &result {
            Ok(Some(_)) => metrics::counter!("cache_hits_total").increment(1),
            Ok(None) => metrics::counter!("cache_misses_total").increment(1),
            Err(_) => {}
        }
        record_cache("get", started, result.is_ok());

        result
    }

    async fn set(&self, user: &User) -> Result<(), CacheError> {
        let key = Self::key(user.id);
        let raw = serde_json::to_string(&CachedUser::from(user))
            .map_err(|e| CacheError::Payload(format!("failed to serialize user: {e}")))?;
        let ttl_secs = self.ttl.as_secs().max(1);
        let mut connection = self.conn.clone();

        let started = Instant::now();
        let result = connection
            .set_ex(key.as_str(), raw.as_str(), ttl_secs)
            .await
            .map_err(|e| CacheError::Backend(format!("cache SETEX user:{} failed: {e}", user.id)));

        record_cache("set", started, result.is_ok());
        result
    }

    async fn delete(&self, id: Uuid) -> Result<(), CacheError> {
        let key = Self::key(id);
        let mut connection = self.conn.clone();

        let started = Instant::now();
        let result = connection
            .del(&key)
            .await
            .map_err(|e| CacheError::Backend(format!("cache DEL user:{id} failed: {e}")));

        record_cache("delete", started, result.is_ok());
        result
    }

    async fn ping(&self) -> Result<(), CacheError> {
        let mut connection = self.conn.clone();

        let started = Instant::now();
        let result = redis::cmd("PING")
            .query_async(&mut connection)
            .await
            .map_err(|e| CacheError::Backend(format!("cache PING failed: {e}")))
            .map(|_pong: String| ());

        record_cache("ping", started, result.is_ok());
        result
    }
}
