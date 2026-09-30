use uuid::Uuid;

use crate::services::domain::User as DomainUser;

/// Persistence entity for a user, mapped 1:1 to the `users` table. Owned
/// by the persistence adapters; mapped to the core's domain model at the
/// persistence edge — the two types are never mixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

impl From<&User> for DomainUser {
    fn from(entity: &User) -> Self {
        Self {
            id: entity.id,
            name: entity.name.clone(),
            email: entity.email.clone(),
        }
    }
}
