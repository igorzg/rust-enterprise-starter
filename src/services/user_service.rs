use std::sync::Arc;

use tracing::{info, warn};
use uuid::Uuid;

use crate::services::domain::{CreateUserCommand, User};
use crate::services::errors::{AppError, NotFoundError};
use crate::services::ports::{UserCache, UserRepository};

/// User use cases: business rules (email uniqueness) and the
/// cache-aside read path. Request validation belongs to the API layer;
/// command fields arrive validated and normalized. The service depends
/// only on core types — the ports, the domain model, and the error
/// model — never on adapters.
pub struct UserService {
    repository: Arc<dyn UserRepository>,
    cache: Arc<dyn UserCache>,
}

impl UserService {
    pub fn new(repository: Arc<dyn UserRepository>, cache: Arc<dyn UserCache>) -> Self {
        Self { repository, cache }
    }

    /// Create a user: enforce email uniqueness, persist, and populate
    /// the cache. Uniqueness is enforced atomically by the repository
    /// (a UNIQUE constraint / `ON CONFLICT`), which maps a conflict to
    /// `AppError::Conflict`.
    pub async fn create(&self, command: CreateUserCommand) -> Result<User, AppError> {
        let user = self
            .repository
            .insert(&command.name, &command.email)
            .await?;

        if let Err(error) = self.cache.set(&user).await {
            warn!(user_id = %user.id, %error, "failed to populate cache after create");
        }
        info!(user_id = %user.id, "user created");
        Ok(user)
    }

    /// Fetch a user by id using a cache-aside strategy:
    /// cache hit → return; miss → PostgreSQL → store with TTL → return.
    /// Cache failures degrade gracefully to the database.
    pub async fn get(&self, id: Uuid) -> Result<User, AppError> {
        match self.cache.get(id).await {
            Ok(Some(user)) => return Ok(user),
            Ok(None) => {}
            Err(error) => {
                warn!(user_id = %id, %error, "cache read failed; falling through to database")
            }
        }

        let user = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(AppError::NotFound(NotFoundError::User { id }))?;

        if let Err(error) = self.cache.set(&user).await {
            warn!(user_id = %id, %error, "failed to store user in cache");
        }
        Ok(user)
    }

    /// Delete a user and invalidate its cache entry.
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let deleted = self.repository.delete(id).await?;
        if !deleted {
            return Err(AppError::NotFound(NotFoundError::User { id }));
        }
        if let Err(error) = self.cache.delete(id).await {
            warn!(user_id = %id, %error, "failed to invalidate cache entry on delete");
        }
        info!(user_id = %id, "user deleted");
        Ok(())
    }
}
