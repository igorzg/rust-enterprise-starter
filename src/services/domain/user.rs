//! Domain model and commands for the user use cases — the
//! core's own vocabulary. The persistence adapters map their entity to
//! this type at the persistence edge; the two types are never mixed.

use uuid::Uuid;

/// User as seen by the service layer. The only user type the core knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub email: String,
}

/// Create-user command. The API layer validates and normalizes the
/// fields (format, length, casing) before building the command;
/// services apply business rules only.
#[derive(Debug, Clone)]
pub struct CreateUserCommand {
    pub name: String,
    pub email: String,
}
