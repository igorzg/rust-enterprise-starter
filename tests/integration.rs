//! Integration tests against a real PostgreSQL and Redis.
//!
//! Skipped automatically when `DATABASE_URL` / `REDIS_URL` are not set, so
//! a bare `cargo test` stays hermetic. CI provides both services and
//! applies the migrations before running the suite.

use std::sync::Arc;
use std::time::Duration;

use rstarter::persistence::cache::redis::RedisUserCache;
use rstarter::persistence::database::create_pool;
use rstarter::persistence::database::postgres::PostgresUserRepository;
use rstarter::services::domain::CreateUserCommand;
use rstarter::services::errors::AppError;
use rstarter::services::ports::{UserCache, UserRepository};
use rstarter::services::user_service::UserService;

/// Return the environment variable, or `None` (→ skip the test) when it is
/// absent or empty.
fn env_or_skip(key: &str) -> Option<String> {
    match std::env::var(key) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!("skipping integration test: {key} not set");
            None
        }
    }
}

/// Wire the real PostgreSQL repository and Redis cache. The environment
/// variables must be set (otherwise the caller skips); a connection
/// failure with variables set is a real failure.
async fn real_service() -> Option<(Arc<UserService>, redis::Client)> {
    let database_url = env_or_skip("DATABASE_URL")?;
    let redis_url = env_or_skip("REDIS_URL")?;

    let pool = create_pool(&database_url)
        .await
        .expect("DATABASE_URL is set but PostgreSQL is unreachable");
    let cache = RedisUserCache::connect(&redis_url, Duration::from_secs(300))
        .await
        .expect("REDIS_URL is set but Redis is unreachable");
    let client = redis::Client::open(redis_url).expect("invalid REDIS_URL");

    let repository: Arc<dyn UserRepository> = Arc::new(PostgresUserRepository::new(pool));
    let user_cache: Arc<dyn UserCache> = Arc::new(cache);
    let service = Arc::new(UserService::new(repository, user_cache));

    Some((service, client))
}

fn unique_email() -> String {
    format!("it-{}@example.com", uuid::Uuid::new_v4())
}

/// The `ON CONFLICT (email)` path: concurrent inserts of the same email
/// must yield exactly one success and one conflict — no race, no error
/// swallowed into a wrong result.
#[tokio::test]
async fn concurrent_inserts_of_same_email_are_atomic() {
    let Some((service, _)) = real_service().await else {
        return;
    };
    let email = unique_email();

    let (a, b) = tokio::join!(
        service.create(CreateUserCommand {
            name: "First".to_string(),
            email: email.clone()
        }),
        service.create(CreateUserCommand {
            name: "Second".to_string(),
            email
        })
    );

    match (a, b) {
        (Ok(_), Err(AppError::Conflict(_))) | (Err(AppError::Conflict(_)), Ok(_)) => {}
        other => panic!("expected one Ok and one Conflict, got {other:?}"),
    }
}

/// Full round trip through the real backends: create → read → verify the
/// Redis payload and TTL → delete → 404 → cache entry gone.
#[tokio::test]
async fn create_get_delete_roundtrip_uses_redis() {
    let Some((service, client)) = real_service().await else {
        return;
    };
    let user = service
        .create(CreateUserCommand {
            name: "Integration".to_string(),
            email: unique_email(),
        })
        .await
        .expect("create failed");

    let fetched = service.get(user.id).await.expect("get failed");
    assert_eq!(fetched, user);

    // The cache fill (on create) must have written the JSON payload with TTL.
    let key = format!("user:{}", user.id);
    let mut conn = client
        .get_multiplexed_async_connection()
        .await
        .expect("redis connection failed");
    let raw: Option<String> = redis::cmd("GET")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .expect("redis GET failed");
    assert!(raw.is_some(), "expected a cached entry at {key}");
    let ttl: i64 = redis::cmd("TTL")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .expect("redis TTL failed");
    assert!(ttl > 0, "expected a positive TTL on {key}, got {ttl}");

    service.delete(user.id).await.expect("delete failed");
    let error = service.get(user.id).await.expect_err("expected NotFound");
    assert!(matches!(error, AppError::NotFound(_)));

    let raw: Option<String> = redis::cmd("GET")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .expect("redis GET failed");
    assert!(raw.is_none(), "cache entry must be invalidated on delete");
}
