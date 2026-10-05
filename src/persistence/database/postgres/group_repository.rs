use std::time::Instant;

use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::helpers::{group_mapping_error, internal_mapping_error, map_db_error, record_db};
use crate::persistence::database::domain::{Group as GroupEntity, User as UserEntity};
use crate::services::domain::{Group, User};
use crate::services::errors::{AppError, ConflictError};
use crate::services::ports::GroupRepository;

/// PostgreSQL implementation of the core's group repository port, including
/// the many-to-many membership operations on the `user_groups` join table.
/// Rows are mapped to persistence entities first, then to the core domain
/// model — the entity never leaves this adapter.
///
/// The `operation` label values are prefixed `group_` so they never collide
/// with the user repository's unprefixed operations under the same
/// `operation` metric label.
pub struct PostgresGroupRepository {
    pool: PgPool,
}

impl PostgresGroupRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn group_from_row(row: &PgRow) -> Result<Group, AppError> {
        let entity = GroupEntity {
            id: row.try_get("id").map_err(group_mapping_error)?,
            name: row.try_get("name").map_err(group_mapping_error)?,
        };
        Ok(Group::from(&entity))
    }

    /// Map a `users` row (selected by a membership join) to the core
    /// `User` — the same fields the user repository selects.
    fn user_from_row(row: &PgRow) -> Result<User, AppError> {
        let entity = UserEntity {
            id: row.try_get("id").map_err(internal_mapping_error)?,
            name: row.try_get("name").map_err(internal_mapping_error)?,
            email: row.try_get("email").map_err(internal_mapping_error)?,
        };
        Ok(User::from(&entity))
    }
}

#[async_trait]
impl GroupRepository for PostgresGroupRepository {
    async fn insert(&self, name: &str) -> Result<Group, AppError> {
        let started = Instant::now();
        // `ON CONFLICT DO NOTHING` makes name uniqueness atomic, mirroring
        // the user repository's email handling.
        let result = sqlx::query(
            "INSERT INTO groups (name) VALUES ($1) \
             ON CONFLICT (name) DO NOTHING \
             RETURNING id, name",
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)
        .and_then(|row| match row {
            Some(row) => Self::group_from_row(&row),
            None => Err(AppError::Conflict(ConflictError::GroupNameAlreadyExists {
                name: name.to_string(),
            })),
        });

        // A duplicate-name conflict is a business outcome (409), not a
        // database failure — only `AppError::Database` counts.
        record_db(
            "group_insert",
            started,
            matches!(&result, Err(AppError::Database(_))),
        );
        result
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Group>, AppError> {
        let started = Instant::now();
        let result = sqlx::query("SELECT id, name FROM groups WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)
            .and_then(|row| row.as_ref().map(Self::group_from_row).transpose());

        record_db("group_find_by_id", started, result.is_err());
        result
    }

    async fn find_all(&self) -> Result<Vec<Group>, AppError> {
        let started = Instant::now();
        let result = sqlx::query("SELECT id, name FROM groups ORDER BY created_at, id")
            .fetch_all(&self.pool)
            .await
            .map_err(map_db_error)
            .and_then(|rows| {
                rows.iter()
                    .map(Self::group_from_row)
                    .collect::<Result<Vec<_>, _>>()
            });

        record_db("group_find_all", started, result.is_err());
        result
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        let started = Instant::now();
        let result = sqlx::query("DELETE FROM groups WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)
            .map(|exec| exec.rows_affected() > 0);

        record_db("group_delete", started, result.is_err());
        result
    }

    async fn add_member(&self, group_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let started = Instant::now();
        // Idempotent: re-assigning an existing member is a no-op.
        let result = sqlx::query(
            "INSERT INTO user_groups (user_id, group_id) VALUES ($1, $2) \
             ON CONFLICT (user_id, group_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(group_id)
        .execute(&self.pool)
        .await
        .map_err(map_db_error)
        .map(|_| ());

        record_db("group_add_member", started, result.is_err());
        result
    }

    async fn remove_member(&self, group_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        let started = Instant::now();
        let result = sqlx::query("DELETE FROM user_groups WHERE user_id = $1 AND group_id = $2")
            .bind(user_id)
            .bind(group_id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)
            .map(|exec| exec.rows_affected() > 0);

        record_db("group_remove_member", started, result.is_err());
        result
    }

    async fn list_members(&self, group_id: Uuid) -> Result<Vec<User>, AppError> {
        let started = Instant::now();
        let result = sqlx::query(
            "SELECT u.id, u.name, u.email \
             FROM users u \
             JOIN user_groups ug ON ug.user_id = u.id \
             WHERE ug.group_id = $1 \
             ORDER BY u.name",
        )
        .bind(group_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)
        .and_then(|rows| {
            rows.iter()
                .map(Self::user_from_row)
                .collect::<Result<Vec<_>, _>>()
        });

        record_db("group_list_members", started, result.is_err());
        result
    }

    async fn groups_for_user(&self, user_id: Uuid) -> Result<Vec<Group>, AppError> {
        let started = Instant::now();
        let result = sqlx::query(
            "SELECT g.id, g.name \
             FROM groups g \
             JOIN user_groups ug ON ug.group_id = g.id \
             WHERE ug.user_id = $1 \
             ORDER BY g.name",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map_err(map_db_error)
        .and_then(|rows| {
            rows.iter()
                .map(Self::group_from_row)
                .collect::<Result<Vec<_>, _>>()
        });

        record_db("group_groups_for_user", started, result.is_err());
        result
    }
}
