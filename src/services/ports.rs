//! Output ports owned by the core. They are expressed in the core's own
//! vocabulary — the domain model and the core error types — never in
//! adapter types. The persistence layer (and the test fakes) implement
//! them; the dependency points inward, from adapter to core.

use async_trait::async_trait;
use uuid::Uuid;

use crate::services::domain::{Group, User};
use crate::services::errors::{AppError, CacheError};

/// Persistence port for users. The production implementation is
/// PostgreSQL (SQLx); tests provide an in-memory fake.
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Insert a new user and return it (with its generated id).
    ///
    /// Returns [`AppError::Conflict`] when the email already exists.
    async fn insert(&self, name: &str, email: &str) -> Result<User, AppError>;

    /// Look up a user by primary key.
    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, AppError>;

    /// Look up a user by email.
    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError>;

    /// Delete a user by primary key. Returns `true` when a row was removed.
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;
}

/// Cache port for users. The production implementation is Redis; the
/// service layer never sees Redis-specific types.
#[async_trait]
pub trait UserCache: Send + Sync {
    /// Return the cached user, if any.
    async fn get(&self, id: Uuid) -> Result<Option<User>, CacheError>;

    /// Store the user with the configured TTL.
    async fn set(&self, user: &User) -> Result<(), CacheError>;

    /// Remove the user from the cache.
    async fn delete(&self, id: Uuid) -> Result<(), CacheError>;

    /// Cheap liveness probe used by the readiness check.
    async fn ping(&self) -> Result<(), CacheError>;
}

/// Persistence port for groups, including the many-to-many membership
/// operations on the `user_groups` join table. The production
/// implementation is PostgreSQL (SQLx); tests provide an in-memory fake.
#[async_trait]
pub trait GroupRepository: Send + Sync {
    /// Insert a new group and return it (with its generated id).
    ///
    /// Returns [`AppError::Conflict`] when the name already exists.
    async fn insert(&self, name: &str) -> Result<Group, AppError>;

    /// Look up a group by primary key.
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Group>, AppError>;

    /// List every group.
    async fn find_all(&self) -> Result<Vec<Group>, AppError>;

    /// Delete a group by primary key. Returns `true` when a row was
    /// removed. Membership rows are removed by the database
    /// (`ON DELETE CASCADE`).
    async fn delete(&self, id: Uuid) -> Result<bool, AppError>;

    /// Assign a user to a group. Idempotent.
    async fn add_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), AppError>;

    /// Remove a user from a group. Returns `true` when a membership row was
    /// removed.
    async fn remove_member(&self, group_id: Uuid, user_id: Uuid) -> Result<bool, AppError>;

    /// List the full users belonging to a group.
    async fn list_members(&self, group_id: Uuid) -> Result<Vec<User>, AppError>;

    /// List the groups a user belongs to.
    async fn groups_for_user(&self, user_id: Uuid) -> Result<Vec<Group>, AppError>;
}

/// Cache port for groups (cache-aside reads), mirroring [`UserCache`]. The
/// production implementation is Redis; the service layer never sees
/// Redis-specific types.
#[async_trait]
pub trait GroupCache: Send + Sync {
    /// Return the cached group, if any.
    async fn get(&self, id: Uuid) -> Result<Option<Group>, CacheError>;

    /// Store the group with the configured TTL.
    async fn set(&self, group: &Group) -> Result<(), CacheError>;

    /// Remove the group from the cache.
    async fn delete(&self, id: Uuid) -> Result<(), CacheError>;

    /// Cheap liveness probe used by the readiness check.
    async fn ping(&self) -> Result<(), CacheError>;
}

/// Status of a single dependency, reported by the adapter that owns it.
/// The core names no specific technology: the adapter supplies the
/// component name (e.g. "postgres", "redis").
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ComponentStatus {
    pub name: &'static str,
    pub up: bool,
}

/// Result of a readiness check: a list of component statuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthStatus {
    pub components: Vec<ComponentStatus>,
}

impl HealthStatus {
    /// The API is ready when every dependency is up.
    pub fn ready(&self) -> bool {
        self.components.iter().all(|component| component.up)
    }
}

/// Dependency-probe port used by the `/ready` endpoint.
#[async_trait]
pub trait HealthCheck: Send + Sync {
    async fn check(&self) -> HealthStatus;
}
