use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::services::domain::User;

/// Cache-specific serialization shape, kept separate from the domain
/// [`User`] so the cache format can evolve independently of the domain
/// model (e.g. adding a schema version field later).
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedUser {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

impl From<&User> for CachedUser {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            name: user.name.clone(),
            email: user.email.clone(),
        }
    }
}

impl From<CachedUser> for User {
    fn from(cached: CachedUser) -> Self {
        Self {
            id: cached.id,
            name: cached.name,
            email: cached.email,
        }
    }
}
