use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::services::domain::CreateGroupCommand;
use crate::services::errors::{AppError, ValidationError};

const MAX_GROUP_NAME_LEN: usize = 120;

/// Request body for creating a group.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateGroupRequest {
    #[schema(max_length = 120, example = "administrators")]
    pub name: String,
}

impl CreateGroupRequest {
    /// Validate the request and map it to the application-layer command.
    /// Request validation (whitespace, length) lives in the API layer;
    /// services apply business rules only (name uniqueness).
    pub fn to_command(&self) -> Result<CreateGroupCommand, AppError> {
        let name = self.name.trim();
        if name.is_empty() || name.len() > MAX_GROUP_NAME_LEN {
            return Err(AppError::Validation(ValidationError::GroupNameLength {
                max: MAX_GROUP_NAME_LEN,
            }));
        }
        Ok(CreateGroupCommand {
            name: name.to_string(),
        })
    }
}

/// Request body for assigning a user to a group.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignGroupMemberRequest {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Uuid,
}
