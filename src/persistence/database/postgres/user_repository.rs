use std::time::Instant;

use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::helpers::{internal_mapping_error, map_db_error, record_db};
use crate::persistence::database::domain::User as UserEntity;
use crate::services::domain::User;
use crate::services::errors::{AppError, ConflictError};
use crate::services::ports::UserRepository;

/// PostgreSQL implementation of the core's user repository port.
/// Rows are mapped to the persistence entity first, then to the core's
/// domain model — the entity never leaves this adapter.
pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Map a `users` row to the persistence entity, then to the core
    /// domain model — the entity never leaves this adapter.
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
impl UserRepository for PostgresUserRepository {
    async fn insert(&self, name: &str, email: &str) -> Result<User, AppError> {
        let started = Instant::now();
        // `ON CONFLICT DO NOTHING` makes uniqueness atomic: a concurrent
        // insert of the same email returns no row instead of racing, so
        // the returned `None` is mapped to `Conflict` below.
        let result = sqlx::query(
            "INSERT INTO users (name, email) VALUES ($1, $2) \
             ON CONFLICT (email) DO NOTHING \
             RETURNING id, name, email",
        )
        .bind(name)
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(map_db_error)
        .and_then(|row| match row {
            Some(row) => Self::user_from_row(&row),
            None => Err(AppError::Conflict(ConflictError::EmailAlreadyExists {
                email: email.to_string(),
            })),
        });

        // A duplicate-email conflict is a business outcome (409), not a
        // database failure — only `AppError::Database` counts.
        record_db(
            "insert",
            started,
            matches!(&result, Err(AppError::Database(_))),
        );
        result
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, AppError> {
        let started = Instant::now();
        let result = sqlx::query("SELECT id, name, email FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)
            .and_then(|row| row.as_ref().map(Self::user_from_row).transpose());

        record_db("find_by_id", started, result.is_err());
        result
    }

    async fn find_by_email(&self, email: &str) -> Result<Option<User>, AppError> {
        let started = Instant::now();
        let result = sqlx::query("SELECT id, name, email FROM users WHERE email = $1")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_db_error)
            .and_then(|row| row.as_ref().map(Self::user_from_row).transpose());

        record_db("find_by_email", started, result.is_err());
        result
    }

    async fn delete(&self, id: Uuid) -> Result<bool, AppError> {
        let started = Instant::now();
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(map_db_error)
            .map(|exec| exec.rows_affected() > 0);

        record_db("delete", started, result.is_err());
        result
    }
}
