//! Free helpers for the Postgres adapter: error mapping and database
//! metrics. Kept out of `user_repository.rs` so that file holds only the
//! repository implementation.

use std::time::Instant;

use crate::services::errors::{AppError, DatabaseError};

pub(super) fn internal_mapping_error(error: sqlx::Error) -> AppError {
    AppError::Database(DatabaseError::Payload(format!(
        "unexpected row shape in users table: {error}"
    )))
}

pub(super) fn map_db_error(error: sqlx::Error) -> AppError {
    AppError::Database(DatabaseError::Backend(error.to_string()))
}

/// Record RED-style metrics for a single database operation.
pub(super) fn record_db(operation: &'static str, started: Instant, failed: bool) {
    metrics::counter!("db_queries_total", "operation" => operation).increment(1);
    metrics::histogram!("db_query_duration_seconds", "operation" => operation)
        .record(started.elapsed().as_secs_f64());
    if failed {
        metrics::counter!("db_errors_total", "operation" => operation).increment(1);
    }
}
