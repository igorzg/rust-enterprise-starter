use async_trait::async_trait;
use uuid::Uuid;

use crate::services::domain::User;
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
