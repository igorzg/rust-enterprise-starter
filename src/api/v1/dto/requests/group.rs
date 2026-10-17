use serde::Deserialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api::validations;
use crate::services::domain::CreateGroupCommand;
use crate::services::errors::AppError;

/// Request body for creating a group.
#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateGroupRequest {
    #[schema(max_length = 120, example = "administrators")]
    pub name: String,
}

impl CreateGroupRequest {
    /// Validate the request and map it to the application-layer command.
    /// Request validation (whitespace, length) lives in
    /// [`crate::api::validations`]; services apply business rules only
    /// (name uniqueness).
    pub fn to_command(&self) -> Result<CreateGroupCommand, AppError> {
        Ok(CreateGroupCommand {
            name: validations::validate_group_name(&self.name)?,
        })
    }
}

/// Request body for assigning a user to a group.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AssignGroupMemberRequest {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub user_id: Uuid,
}
