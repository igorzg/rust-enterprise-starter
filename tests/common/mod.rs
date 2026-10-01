//! In-memory fakes for the core's output ports, shared by the test
//! suites.
//!
//! Counters (db_reads, hits, misses) let tests prove the cache-aside
//! behavior: a GET must hit the database exactly once and the cache on
//! subsequent reads.
#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use uuid::Uuid;

use rstarter::services::domain::User;
use rstarter::services::errors::{AppError, CacheError, ConflictError, DatabaseError};
use rstarter::services::ports::{
    ComponentStatus, HealthCheck, HealthStatus, UserCache, UserRepository,
};
use rstarter::services::user_service::UserService;
use rstarter::state::AppState;

/// In-memory user repository.
pub struct InMemoryUserRepository {
    users: Mutex<HashMap<Uuid, User>>,
    pub db_reads: AtomicUsize,
}

impl Default for InMemoryUserRepository {
    fn default() -> Self {
        Self {
            users: Mutex::new(HashMap::new()),
            db_reads: AtomicUsize::new(0),
        }
    }
}

impl InMemoryUserRepository {
    pub fn seed(&self, user: User) {
        self.users.lock().unwrap().insert(user.id, user);
    }
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn insert(&self, name: &str, email: &str) -> Result<User, AppError> {
        if self.find_by_email(email).await?.is_some() {
            return Err(AppError::Conflict(ConflictError::EmailAlreadyExists {
                email: email.to_string(),
            }));
        }
        // Mirror Postgres `gen_random_uuid()`: the repository assigns the id.
        let user = User {
            id: Uuid::new_v4(),
            name: name.to_string(),
            email: email.to_string(),
        };
        self.users.lock().unwrap().insert(user.id, user.clone());
        Ok(user)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, AppError> {
        self.db_reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.users.lock().unwrap().get(&id).cloned())
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .values()
            .find(|user| user.email == email)
            .cloned())
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        Ok(self.users.lock().unwrap().remove(&id).is_some())
    }
}

/// In-memory user cache with hit/miss counters.
#[derive(Default)]
pub struct InMemoryUserCache {
    entries: Mutex<HashMap<Uuid, User>>,
    pub hits: AtomicUsize,
    pub misses: AtomicUsize,
}

#[async_trait]
impl UserCache for InMemoryUserCache {
    async fn get(&self, id: Uuid) -> Result<Option<User>, CacheError> {
        let found = self.entries.lock().unwrap().get(&id).cloned();
        if found.is_some() {
            self.hits.fetch_add(1, Ordering::SeqCst);
        } else {
            self.misses.fetch_add(1, Ordering::SeqCst);
        }
        Ok(found)
    }

    async fn set(&self, user: &User) -> Result<(), CacheError> {
        self.entries.lock().unwrap().insert(user.id, user.clone());
        Ok(())
    }

    async fn delete(&self, id: Uuid) -> Result<(), CacheError> {
        self.entries.lock().unwrap().remove(&id);
        Ok(())
    }

    async fn ping(&self) -> Result<(), CacheError> {
        Ok(())
    }
}

/// Health probe whose readiness is controlled by the test.
#[derive(Default)]
pub struct InMemoryHealthCheck {
    pub ready: bool,
}

#[async_trait]
impl HealthCheck for InMemoryHealthCheck {
    async fn check(&self) -> HealthStatus {
        HealthStatus {
            components: vec![
                ComponentStatus {
                    name: "postgres",
                    up: self.ready,
                },
                ComponentStatus {
                    name: "redis",
                    up: self.ready,
                },
            ],
        }
    }
}

/// Build a fully wired [`AppState`] with in-memory fakes.
pub fn test_state(
    healthy: bool,
) -> (
    AppState,
    Arc<InMemoryUserRepository>,
    Arc<InMemoryUserCache>,
) {
    // Install the Prometheus recorder once per test process so the
    // `/metrics` endpoint and HTTP middleware work under test.
    rstarter::api::metrics::init();

    let repository = Arc::new(InMemoryUserRepository::default());
    let cache = Arc::new(InMemoryUserCache::default());
    let state = AppState {
        user_service: Arc::new(UserService::new(repository.clone(), cache.clone())),
        health: Arc::new(InMemoryHealthCheck { ready: healthy }),
    };
    (state, repository, cache)
}

/// A repository whose every operation fails — for 5xx error tests.
pub struct FailingUserRepository;

impl FailingUserRepository {
    fn failure() -> AppError {
        AppError::Database(DatabaseError::Backend(
            "simulated database failure: on fire".to_string(),
        ))
    }
}

#[async_trait]
impl UserRepository for FailingUserRepository {
    async fn insert(&self, _name: &str, _email: &str) -> Result<User, AppError> {
        Err(Self::failure())
    }

    async fn find_by_id(&self, _id: Uuid) -> Result<Option<User>, AppError> {
        Err(Self::failure())
    }

    async fn find_by_email(&self, _email: &str) -> Result<Option<User>, AppError> {
        Err(Self::failure())
    }

    async fn delete(&self, _id: Uuid) -> Result<bool, AppError> {
        Err(Self::failure())
    }
}

/// Build an [`AppState`] whose repository fails every operation — for
/// 5xx error-envelope and log tests.
pub fn failing_state() -> AppState {
    rstarter::api::metrics::init();

    AppState {
        user_service: Arc::new(UserService::new(
            Arc::new(FailingUserRepository),
            Arc::new(InMemoryUserCache::default()),
        )),
        health: Arc::new(InMemoryHealthCheck { ready: true }),
    }
}
