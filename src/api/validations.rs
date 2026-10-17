//! API-layer request validation. All format, length, and normalization
//! checks for request fields live here, version-agnostic; request DTOs
//! call into this module when mapping to commands, so services receive
//! already-validated, normalized command fields and hold business rules
//! only.

use crate::services::errors::{AppError, ValidationError};

const MAX_NAME_LEN: usize = 120;
const MAX_EMAIL_LEN: usize = 254;
const MAX_GROUP_NAME_LEN: usize = 120;

/// Trim and validate a user's display name.
pub fn validate_user_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() || name.len() > MAX_NAME_LEN {
        return Err(AppError::Validation(ValidationError::NameLength {
            max: MAX_NAME_LEN,
        }));
    }
    Ok(name.to_string())
}

/// Trim, lowercase, and validate an email address.
pub fn validate_email(email: &str) -> Result<String, AppError> {
    let email = email.trim().to_ascii_lowercase();
    let len = email.len();
    if len == 0 || len > MAX_EMAIL_LEN {
        return Err(AppError::Validation(ValidationError::EmailLength {
            max: MAX_EMAIL_LEN,
        }));
    }
    let at = match email.find('@') {
        Some(at) if email.matches('@').count() == 1 => at,
        _ => return Err(AppError::Validation(ValidationError::EmailAtSign)),
    };
    let local = &email[..at];
    let domain = &email[at + 1..];
    if local.is_empty() || domain.len() < 3 || !domain.contains('.') {
        return Err(AppError::Validation(ValidationError::EmailLocalDomain));
    }
    if domain.starts_with('.') || domain.ends_with('.') || domain.contains("..") {
        return Err(AppError::Validation(ValidationError::EmailDomainDots));
    }
    Ok(email)
}

/// Trim and validate a group name.
pub fn validate_group_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() || name.len() > MAX_GROUP_NAME_LEN {
        return Err(AppError::Validation(ValidationError::GroupNameLength {
            max: MAX_GROUP_NAME_LEN,
        }));
    }
    Ok(name.to_string())
}
