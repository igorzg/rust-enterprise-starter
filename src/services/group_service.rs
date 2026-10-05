use std::sync::Arc;

use tracing::{info, warn};
use uuid::Uuid;

use crate::services::domain::{CreateGroupCommand, Group, User};
use crate::services::errors::{AppError, NotFoundError};
use crate::services::ports::{GroupCache, GroupRepository, UserRepository};

/// Group use cases: group CRUD and many-to-many membership with users.
/// Reads use a cache-aside strategy (mirroring
/// [`super::user_service::UserService`]). Request validation belongs to the
/// API layer; command fields arrive validated and normalized. The service
/// depends only on core types.
pub struct GroupService {
    repository: Arc<dyn GroupRepository>,
    user_repository: Arc<dyn UserRepository>,
    cache: Arc<dyn GroupCache>,
}

impl GroupService {
    pub fn new(
        repository: Arc<dyn GroupRepository>,
        user_repository: Arc<dyn UserRepository>,
        cache: Arc<dyn GroupCache>,
    ) -> Self {
        Self {
            repository,
            user_repository,
            cache,
        }
    }

    /// Create a group: enforce name uniqueness, persist, and populate the
    /// cache. Uniqueness is enforced atomically by the repository (a UNIQUE
    /// constraint / `ON CONFLICT`), which maps a conflict to
    /// `AppError::Conflict`.
    pub async fn create(&self, command: CreateGroupCommand) -> Result<Group, AppError> {
        let group = self.repository.insert(&command.name).await?;

        if let Err(error) = self.cache.set(&group).await {
            warn!(group_id = %group.id, %error, "failed to populate cache after create");
        }
        info!(group_id = %group.id, "group created");
        Ok(group)
    }

    /// List all groups, oldest first.
    pub async fn list(&self) -> Result<Vec<Group>, AppError> {
        self.repository.find_all().await
    }

    /// Fetch a group by id using a cache-aside strategy: cache hit → return;
    /// miss → PostgreSQL → store with TTL → return. Cache failures degrade
    /// gracefully to the database.
    pub async fn get(&self, id: Uuid) -> Result<Group, AppError> {
        match self.cache.get(id).await {
            Ok(Some(group)) => return Ok(group),
            Ok(None) => {}
            Err(error) => {
                warn!(group_id = %id, %error, "cache read failed; falling through to database")
            }
        }

        let group = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or(AppError::NotFound(NotFoundError::Group { id }))?;

        if let Err(error) = self.cache.set(&group).await {
            warn!(group_id = %id, %error, "failed to store group in cache");
        }
        Ok(group)
    }

    /// Fetch a group by id together with its current members. The group is
    /// cache-aside; members are read fresh from the database (membership
    /// changes often and is intentionally not cached).
    pub async fn get_with_members(&self, id: Uuid) -> Result<(Group, Vec<User>), AppError> {
        let group = self.get(id).await?;
        let members = self.repository.list_members(id).await?;
        Ok((group, members))
    }

    /// Delete a group and invalidate its cache entry. Membership rows are
    /// removed by the database (`ON DELETE CASCADE` on the join table).
    pub async fn delete(&self, id: Uuid) -> Result<(), AppError> {
        let deleted = self.repository.delete(id).await?;
        if !deleted {
            return Err(AppError::NotFound(NotFoundError::Group { id }));
        }
        if let Err(error) = self.cache.delete(id).await {
            warn!(group_id = %id, %error, "failed to invalidate cache entry on delete");
        }
        info!(group_id = %id, "group deleted");
        Ok(())
    }

    /// Assign a user to a group. Both must exist. Idempotent: assigning a
    /// user who is already a member is a no-op success.
    pub async fn add_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        self.require_group(group_id).await?;
        self.require_user(user_id).await?;
        self.repository.add_member(group_id, user_id).await?;
        info!(group_id = %group_id, user_id = %user_id, "user assigned to group");
        Ok(())
    }

    /// Remove a user from a group. Returns `NotFound` when the group or the
    /// user does not exist, or when the user was not a member.
    pub async fn remove_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        self.require_group(group_id).await?;
        self.require_user(user_id).await?;
        let removed = self.repository.remove_member(group_id, user_id).await?;
        if !removed {
            return Err(AppError::NotFound(NotFoundError::GroupMembership {
                group_id,
                user_id,
            }));
        }
        info!(group_id = %group_id, user_id = %user_id, "user removed from group");
        Ok(())
    }

    /// List the groups a user belongs to. The user must exist.
    pub async fn groups_for_user(&self, user_id: Uuid) -> Result<Vec<Group>, AppError> {
        self.require_user(user_id).await?;
        self.repository.groups_for_user(user_id).await
    }

    async fn require_group(&self, id: Uuid) -> Result<(), AppError> {
        self.repository
            .find_by_id(id)
            .await?
            .ok_or(AppError::NotFound(NotFoundError::Group { id }))?;
        Ok(())
    }

    async fn require_user(&self, id: Uuid) -> Result<(), AppError> {
        self.user_repository
            .find_by_id(id)
            .await?
            .ok_or(AppError::NotFound(NotFoundError::User { id }))?;
        Ok(())
    }
}
