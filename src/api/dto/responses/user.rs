use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::services::domain::User;

/// User representation returned by the API.
#[derive(Debug, Serialize, ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

impl From<&User> for UserResponse {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            name: user.name.clone(),
            email: user.email.clone(),
        }
    }
}
