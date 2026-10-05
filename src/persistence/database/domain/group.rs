use uuid::Uuid;

use crate::services::domain::Group as DomainGroup;

/// Persistence entity for a group, mapped 1:1 to the `groups` table. Owned
/// by the persistence adapters; mapped to the core's domain model at the
/// persistence edge — the two types are never mixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: Uuid,
    pub name: String,
}

impl From<&Group> for DomainGroup {
    fn from(entity: &Group) -> Self {
        Self {
            id: entity.id,
            name: entity.name.clone(),
        }
    }
}
