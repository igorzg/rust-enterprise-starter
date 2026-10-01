use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;
use utoipa::ToSchema;

use crate::api::i18n;
use crate::services::errors::{AppError, ConflictError, NotFoundError, ValidationError};

/// Machine-readable error code plus a client-safe message.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

/// Consistent JSON error envelope: `{"error": {"code": "...", "message": "..."}}`.
#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorResponse {
    pub error: ErrorBody,
}

impl ErrorResponse {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ErrorBody {
                code: code.into(),
                message: message.into(),
            },
        }
    }
}

impl AppError {
    /// Map the application error to an HTTP status code.
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Database(_) | Self::Cache(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Render the client-facing message in the given locale.
    pub fn localized_message(&self, locale: &str) -> String {
        match self {
            AppError::Validation(ValidationError::NameLength { max }) => {
                t!("validation.name_length", locale = locale, max = *max).into_owned()
            }
            AppError::Validation(ValidationError::EmailLength { max }) => {
                t!("validation.email_length", locale = locale, max = *max).into_owned()
            }
            AppError::Validation(ValidationError::EmailAtSign) => {
                t!("validation.email_at_sign", locale = locale).into_owned()
            }
            AppError::Validation(ValidationError::EmailLocalDomain) => {
                t!("validation.email_local_domain", locale = locale).into_owned()
            }
            AppError::Validation(ValidationError::EmailDomainDots) => {
                t!("validation.email_domain_dots", locale = locale).into_owned()
            }
            AppError::NotFound(NotFoundError::User { id }) => {
                t!("errors.user_not_found", locale = locale, id = *id).into_owned()
            }
            AppError::Conflict(ConflictError::EmailAlreadyExists { email }) => t!(
                "errors.email_already_exists",
                locale = locale,
                email = email.as_str()
            )
            .into_owned(),
            AppError::Database(_) | AppError::Cache(_) => {
                t!("errors.internal_error", locale = locale).into_owned()
            }
        }
    }
}

/// Error raised at the HTTP boundary: an invalid path parameter or body,
/// an unknown route, or any core application error. The HTTP-only variants
/// (invalid id/body, route-not-found) live here, not in the core's
/// [`AppError`].
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("request body is not valid JSON")]
    InvalidBody,

    #[error("id is not a valid UUID")]
    InvalidId,

    #[error("route not found")]
    RouteNotFound,

    #[error(transparent)]
    App(#[from] AppError),
}

impl ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::InvalidBody | Self::InvalidId => StatusCode::BAD_REQUEST,
            Self::RouteNotFound => StatusCode::NOT_FOUND,
            Self::App(app) => app.status_code(),
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::InvalidBody | Self::InvalidId => "VALIDATION_ERROR",
            Self::RouteNotFound => "NOT_FOUND",
            Self::App(app) => app.code(),
        }
    }

    fn localized_message(&self, locale: &str) -> String {
        match self {
            Self::InvalidBody => t!("validation.invalid_body", locale = locale).into_owned(),
            Self::InvalidId => t!("validation.invalid_id", locale = locale).into_owned(),
            Self::RouteNotFound => t!("errors.route_not_found", locale = locale).into_owned(),
            Self::App(app) => app.localized_message(locale),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.code();
        let locale = i18n::current_locale();
        let message = self.localized_message(&locale);

        // 5xx: log the full source chain server-side (including the
        // root cause, e.g. the SQLx error behind AppError::Database),
        // return a sanitized message.
        // 4xx: the message is already client-safe.
        match status {
            StatusCode::INTERNAL_SERVER_ERROR => {
                tracing::error!(code = code, detail = %error_chain(&self), "request failed")
            }
            _ => tracing::warn!(code = code, detail = %self, "request rejected"),
        }

        (status, Json(ErrorResponse::new(code, message))).into_response()
    }
}

/// Join an error and its `source()` chain with `": "` so the root cause
/// reaches the log: `Display` alone only shows the outermost variant
/// (e.g. `AppError::Database` renders as "database error" without the
/// underlying SQLx error).
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut chain: Vec<String> = Vec::new();
    let mut current: Option<&dyn std::error::Error> = Some(error);
    while let Some(err) = current {
        chain.push(err.to_string());
        current = err.source();
    }
    chain.join(": ")
}
