use serde::Deserialize;
use utoipa::ToSchema;

use crate::services::domain::CreateUserCommand;
use crate::services::errors::{AppError, ValidationError};

const MAX_EMAIL_LEN: usize = 254;
const MAX_NAME_LEN: usize = 120;

/// Request body for creating a user.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateUserRequest {
    #[schema(max_length = 120, example = "Jane Doe")]
    pub name: String,
    #[schema(max_length = 254, example = "jane.doe@example.com")]
    pub email: String,
}

impl CreateUserRequest {
    /// Validate the request and map it to the application-layer
    /// command. All request validation (format, length, normalization)
    /// lives here, in the API layer; services apply business rules only.
    pub fn to_command(&self) -> Result<CreateUserCommand, AppError> {
        let name = self.name.trim();
        if name.is_empty() || name.len() > MAX_NAME_LEN {
            return Err(AppError::Validation(ValidationError::NameLength {
                max: MAX_NAME_LEN,
            }));
        }
        let email = self.email.trim().to_ascii_lowercase();
        validate_email(&email)?;
        Ok(CreateUserCommand {
            name: name.to_string(),
            email,
        })
    }
}

fn validate_email(email: &str) -> Result<(), AppError> {
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
    Ok(())
}
