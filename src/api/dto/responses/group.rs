use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api::dto::responses::UserResponse;
use crate::services::domain::{Group, User};

/// Group representation returned by the API (list / create).
#[derive(Debug, Serialize, ToSchema)]
pub struct GroupResponse {
    pub id: Uuid,
    pub name: String,
}

impl From<&Group> for GroupResponse {
    fn from(group: &Group) -> Self {
        Self {
            id: group.id,
            name: group.name.clone(),
        }
    }
}

/// A single group together with its current members (GET /groups/{id}).
/// Members are read fresh from the database; the group itself is
/// cache-aside.
#[derive(Debug, Serialize, ToSchema)]
pub struct GroupDetailResponse {
    pub id: Uuid,
    pub name: String,
    pub members: Vec<UserResponse>,
}

impl GroupDetailResponse {
    pub fn from_parts(group: &Group, members: &[User]) -> Self {
        Self {
            id: group.id,
            name: group.name.clone(),
            members: members.iter().map(UserResponse::from).collect(),
        }
    }
}
