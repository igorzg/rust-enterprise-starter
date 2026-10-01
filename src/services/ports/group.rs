use async_trait::async_trait;
use uuid::Uuid;

use crate::services::domain::{Group, User};
use crate::services::errors::{AppError, CacheError};

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

/// Cache port for groups (cache-aside reads), mirroring
/// [`crate::services::ports::user::UserCache`]. The production
/// implementation is Redis; the service layer never sees Redis-specific
/// types.
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
