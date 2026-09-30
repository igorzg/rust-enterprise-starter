use thiserror::Error;
use uuid::Uuid;

/// Errors returned by the cache port. The Redis implementation lives in
/// the persistence layer; Redis-specific details stay there.
#[derive(Debug, Error)]
pub enum CacheError {
    #[error("cache backend error: {0}")]
    Backend(String),

    #[error("cache payload error: {0}")]
    Payload(String),
}

/// Errors returned by the database port. The SQLx implementation lives in
/// the persistence layer; SQLx-specific details stay there.
#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database backend error: {0}")]
    Backend(String),

    #[error("database payload error: {0}")]
    Payload(String),
}

/// Locale-agnostic validation failure. Carries data, not language: the
/// API layer maps each variant to a localized message. The `Display`
/// output is English and used only for logging.
#[derive(Debug, Clone, Error)]
pub enum ValidationError {
    #[error("name length out of range (max {max})")]
    NameLength { max: usize },

    #[error("email length out of range (max {max})")]
    EmailLength { max: usize },

    #[error("email has no single '@'")]
    EmailAtSign,

    #[error("email local part or domain is invalid")]
    EmailLocalDomain,

    #[error("email domain has invalid dots")]
    EmailDomainDots,
}

/// Locale-agnostic "not found" failure.
#[derive(Debug, Clone, Error)]
pub enum NotFoundError {
    #[error("user with id {id} was not found")]
    User { id: Uuid },
}

/// Locale-agnostic conflict failure.
#[derive(Debug, Clone, Error)]
pub enum ConflictError {
    #[error("a user with email {email} already exists")]
    EmailAlreadyExists { email: String },
}

/// Unified application error model, owned by the core. Services raise the
/// business variants (`NotFound`, `Conflict`), the API adapter raises
/// `Validation` and maps every variant to an HTTP status code and a
/// consistent JSON error envelope, and the persistence adapters raise the
/// infrastructure variants (`Database`, `Cache`). Internal details (SQL
/// errors, connection strings, stack traces) are never exposed to API
/// clients.
///
/// The 4xx variants carry locale-agnostic data; the API layer renders the
/// localized client message. `Display` is English, for logging only.
#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Validation(ValidationError),

    #[error(transparent)]
    NotFound(NotFoundError),

    #[error(transparent)]
    Conflict(ConflictError),

    #[error("database error")]
    Database(#[source] DatabaseError),

    #[error("cache error")]
    Cache(#[source] CacheError),
}

impl AppError {
    /// Stable machine-readable code used in the JSON error envelope.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::NotFound(_) => "NOT_FOUND",
            Self::Conflict(_) => "CONFLICT",
            Self::Database(_) => "DATABASE_ERROR",
            Self::Cache(_) => "CACHE_ERROR",
        }
    }
}
