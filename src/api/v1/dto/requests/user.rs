use serde::Deserialize;
use utoipa::ToSchema;

use crate::api::validations;
use crate::services::domain::CreateUserCommand;
use crate::services::errors::AppError;

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
    /// lives in [`crate::api::validations`]; services apply business
    /// rules only.
    pub fn to_command(&self) -> Result<CreateUserCommand, AppError> {
        Ok(CreateUserCommand {
            name: validations::validate_user_name(&self.name)?,
            email: validations::validate_email(&self.email)?,
        })
    }
}
