use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::services::domain::Group;

/// Cache-specific serialization shape for a group, kept separate from the
/// domain [`Group`] so the cache format can evolve independently of the
/// domain model (e.g. adding a schema version field later).
#[derive(Debug, Serialize, Deserialize)]
pub struct CachedGroup {
    pub id: Uuid,
    pub name: String,
}

impl From<&Group> for CachedGroup {
    fn from(group: &Group) -> Self {
        Self {
            id: group.id,
            name: group.name.clone(),
        }
    }
}

impl From<CachedGroup> for Group {
    fn from(cached: CachedGroup) -> Self {
        Self {
            id: cached.id,
            name: cached.name,
        }
    }
}
