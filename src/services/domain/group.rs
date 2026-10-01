//! Domain model and commands for the group use cases — the core's own
//! vocabulary, mirroring the user domain model.

use uuid::Uuid;

/// Group as seen by the service layer. The only group type the core knows.
///
/// Membership (which users belong to a group) is a separate concern — the
/// `user_groups` join — and is queried through the repository rather than
/// embedded here, so the stored/cached group stays small and stable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: Uuid,
    pub name: String,
}

/// Create-group command. The API layer validates and normalizes the name
/// (whitespace, length) before building the command; the service applies
/// business rules only (name uniqueness).
#[derive(Debug, Clone)]
pub struct CreateGroupCommand {
    pub name: String,
}
