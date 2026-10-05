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

use rstarter::services::domain::{Group, User};
use rstarter::services::errors::{AppError, CacheError, ConflictError, DatabaseError};
use rstarter::services::group_service::GroupService;
use rstarter::services::ports::{
    ComponentStatus, GroupCache, GroupRepository, HealthCheck, HealthStatus, UserCache,
    UserRepository,
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

/// In-memory group repository backed by the same in-memory users, so
/// membership operations and existence checks stay consistent.
pub struct InMemoryGroupRepository {
    groups: Mutex<HashMap<Uuid, Group>>,
    /// Membership rows as `(user_id, group_id)`.
    memberships: Mutex<HashMap<(Uuid, Uuid), ()>>,
    users: Arc<InMemoryUserRepository>,
}

impl InMemoryGroupRepository {
    pub fn new(users: Arc<InMemoryUserRepository>) -> Self {
        Self {
            groups: Mutex::new(HashMap::new()),
            memberships: Mutex::new(HashMap::new()),
            users,
        }
    }

    pub fn seed(&self, group: Group) {
        self.groups.lock().unwrap().insert(group.id, group);
    }

    fn user(&self, id: Uuid) -> Option<User> {
        self.users.users.lock().unwrap().get(&id).cloned()
    }
}

#[async_trait]
impl GroupRepository for InMemoryGroupRepository {
    async fn insert(&self, name: &str) -> Result<Group, AppError> {
        let mut groups = self.groups.lock().unwrap();
        if groups.values().any(|group| group.name == name) {
            return Err(AppError::Conflict(ConflictError::GroupNameAlreadyExists {
                name: name.to_string(),
            }));
        }
        // Mirror Postgres `gen_random_uuid()`: the repository assigns the id.
        let group = Group {
            id: Uuid::new_v4(),
            name: name.to_string(),
        };
        groups.insert(group.id, group.clone());
        Ok(group)
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Group>, AppError> {
        Ok(self.groups.lock().unwrap().get(&id).cloned())
    }

    async fn find_all(&self) -> Result<Vec<Group>, AppError> {
        Ok(self.groups.lock().unwrap().values().cloned().collect())
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        let deleted = self.groups.lock().unwrap().remove(&id).is_some();
        // Mirror `ON DELETE CASCADE` on the join table.
        self.memberships
            .lock()
            .unwrap()
            .retain(|(_, group_id), _| *group_id != id);
        Ok(deleted)
    }

    async fn add_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        self.memberships
            .lock()
            .unwrap()
            .insert((user_id, group_id), ());
        Ok(())
    }

    async fn remove_member(&self, group_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        Ok(self
            .memberships
            .lock()
            .unwrap()
            .remove(&(user_id, group_id))
            .is_some())
    }

    async fn list_members(&self, group_id: Uuid) -> Result<Vec<User>, AppError> {
        let user_ids = self
            .memberships
            .lock()
            .unwrap()
            .keys()
            .filter(|(_, gid)| *gid == group_id)
            .map(|(user_id, _)| *user_id)
            .collect::<Vec<_>>();
        let mut members: Vec<User> = user_ids.iter().filter_map(|id| self.user(*id)).collect();
        members.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(members)
    }

    async fn groups_for_user(&self, user_id: Uuid) -> Result<Vec<Group>, AppError> {
        let group_ids = self
            .memberships
            .lock()
            .unwrap()
            .keys()
            .filter(|(uid, _)| *uid == user_id)
            .map(|(_, group_id)| *group_id)
            .collect::<Vec<_>>();
        let mut groups: Vec<Group> = group_ids
            .iter()
            .filter_map(|id| self.groups.lock().unwrap().get(id).cloned())
            .collect();
        groups.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(groups)
    }
}

/// In-memory group cache with hit/miss counters, mirroring
/// [`InMemoryUserCache`].
#[derive(Default)]
pub struct InMemoryGroupCache {
    entries: Mutex<HashMap<Uuid, Group>>,
    pub hits: AtomicUsize,
    pub misses: AtomicUsize,
}

#[async_trait]
impl GroupCache for InMemoryGroupCache {
    async fn get(&self, id: Uuid) -> Result<Option<Group>, CacheError> {
        let found = self.entries.lock().unwrap().get(&id).cloned();
        if found.is_some() {
            self.hits.fetch_add(1, Ordering::SeqCst);
        } else {
            self.misses.fetch_add(1, Ordering::SeqCst);
        }
        Ok(found)
    }

    async fn set(&self, group: &Group) -> Result<(), CacheError> {
        self.entries.lock().unwrap().insert(group.id, group.clone());
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

/// Build a fully wired [`AppState`] with in-memory fakes, handing back
/// handles to every fake for assertions (cache hits/misses, seeded rows).
fn build_state_full(
    healthy: bool,
) -> (
    AppState,
    Arc<InMemoryUserRepository>,
    Arc<InMemoryUserCache>,
    Arc<InMemoryGroupRepository>,
    Arc<InMemoryGroupCache>,
) {
    // Install the Prometheus recorder once per test process so the
    // `/metrics` endpoint and HTTP middleware work under test.
    rstarter::api::metrics::init();

    let user_repository = Arc::new(InMemoryUserRepository::default());
    let user_cache = Arc::new(InMemoryUserCache::default());
    let group_repository = Arc::new(InMemoryGroupRepository::new(user_repository.clone()));
    let group_cache = Arc::new(InMemoryGroupCache::default());
    let state = AppState {
        user_service: Arc::new(UserService::new(
            user_repository.clone(),
            user_cache.clone(),
        )),
        group_service: Arc::new(GroupService::new(
            group_repository.clone(),
            user_repository.clone(),
            group_cache.clone(),
        )),
        health: Arc::new(InMemoryHealthCheck { ready: healthy }),
    };
    (
        state,
        user_repository,
        user_cache,
        group_repository,
        group_cache,
    )
}

/// Build a fully wired [`AppState`] with in-memory fakes for the user
/// feature (the group handles are dropped).
pub fn test_state(
    healthy: bool,
) -> (
    AppState,
    Arc<InMemoryUserRepository>,
    Arc<InMemoryUserCache>,
) {
    let (state, user_repository, user_cache, _, _) = build_state_full(healthy);
    (state, user_repository, user_cache)
}

/// Build a fully wired [`AppState`] with in-memory fakes for the group
/// feature, exposing the group (and backing user) fakes.
pub fn test_state_full(
    healthy: bool,
) -> (
    AppState,
    Arc<InMemoryUserRepository>,
    Arc<InMemoryUserCache>,
    Arc<InMemoryGroupRepository>,
    Arc<InMemoryGroupCache>,
) {
    build_state_full(healthy)
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

/// A repository whose every group operation fails — for 5xx error tests.
pub struct FailingGroupRepository;

impl FailingGroupRepository {
    fn failure() -> AppError {
        AppError::Database(DatabaseError::Backend(
            "simulated database failure: on fire".to_string(),
        ))
    }
}

#[async_trait]
impl GroupRepository for FailingGroupRepository {
    async fn insert(&self, _name: &str) -> Result<Group, AppError> {
        Err(Self::failure())
    }

    async fn find_by_id(&self, _id: Uuid) -> Result<Option<Group>, AppError> {
        Err(Self::failure())
    }

    async fn find_all(&self) -> Result<Vec<Group>, AppError> {
        Err(Self::failure())
    }

    async fn delete(&self, _id: Uuid) -> Result<bool, AppError> {
        Err(Self::failure())
    }

    async fn add_member(&self, _group_id: Uuid, _user_id: Uuid) -> Result<(), AppError> {
        Err(Self::failure())
    }

    async fn remove_member(&self, _group_id: Uuid, _user_id: Uuid) -> Result<bool, AppError> {
        Err(Self::failure())
    }

    async fn list_members(&self, _group_id: Uuid) -> Result<Vec<User>, AppError> {
        Err(Self::failure())
    }

    async fn groups_for_user(&self, _user_id: Uuid) -> Result<Vec<Group>, AppError> {
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
        group_service: Arc::new(GroupService::new(
            Arc::new(FailingGroupRepository),
            Arc::new(FailingUserRepository),
            Arc::new(InMemoryGroupCache::default()),
        )),
        health: Arc::new(InMemoryHealthCheck { ready: true }),
    }
}
