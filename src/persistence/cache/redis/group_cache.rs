use std::time::{Duration, Instant};

use async_trait::async_trait;
use redis::AsyncCommands;
use uuid::Uuid;

use crate::persistence::cache::domain::CachedGroup;
use crate::services::domain::Group;
use crate::services::errors::CacheError;
use crate::services::ports::GroupCache;

/// Redis implementation of the core's group cache port.
///
/// Redis types never leave this module: values are JSON-serialized
/// [`CachedGroup`] objects keyed by `group:{id}` with a TTL. Operation
/// names are prefixed `group_` to avoid colliding with the user cache's
/// unprefixed operations under the shared `operation` metric label.
pub struct RedisGroupCache {
    conn: redis::aio::MultiplexedConnection,
    ttl: Duration,
}

impl RedisGroupCache {
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
        format!("group:{id}")
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
impl GroupCache for RedisGroupCache {
    async fn get(&self, id: Uuid) -> Result<Option<Group>, CacheError> {
        let key = Self::key(id);
        let mut connection = self.conn.clone();

        let started = Instant::now();
        let raw: Result<Option<String>, CacheError> = connection
            .get(&key)
            .await
            .map_err(|e| CacheError::Backend(format!("cache GET group:{id} failed: {e}")));

        let result = raw.and_then(|raw| match raw {
            None => Ok(None),
            Some(raw) => serde_json::from_str::<CachedGroup>(&raw)
                .map(|cached| Some(Group::from(cached)))
                .map_err(|e| {
                    CacheError::Payload(format!("cached group:{id} is not valid JSON: {e}"))
                }),
        });

        match &result {
            Ok(Some(_)) => metrics::counter!("cache_hits_total").increment(1),
            Ok(None) => metrics::counter!("cache_misses_total").increment(1),
            Err(_) => {}
        }
        record_cache("group_get", started, result.is_ok());

        result
    }

    async fn set(&self, group: &Group) -> Result<(), CacheError> {
        let key = Self::key(group.id);
        let raw = serde_json::to_string(&CachedGroup::from(group))
            .map_err(|e| CacheError::Payload(format!("failed to serialize group: {e}")))?;
        let ttl_secs = self.ttl.as_secs().max(1);
        let mut connection = self.conn.clone();

        let started = Instant::now();
        let result = connection
            .set_ex(key.as_str(), raw.as_str(), ttl_secs)
            .await
            .map_err(|e| {
                CacheError::Backend(format!("cache SETEX group:{} failed: {e}", group.id))
            });

        record_cache("group_set", started, result.is_ok());
        result
    }

    async fn delete(&self, id: Uuid) -> Result<(), CacheError> {
        let key = Self::key(id);
        let mut connection = self.conn.clone();

        let started = Instant::now();
        let result = connection
            .del(&key)
            .await
            .map_err(|e| CacheError::Backend(format!("cache DEL group:{id} failed: {e}")));

        record_cache("group_delete", started, result.is_ok());
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

        record_cache("group_ping", started, result.is_ok());
        result
    }
}
