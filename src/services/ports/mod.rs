//! Output ports owned by the core. They are expressed in the core's own
//! vocabulary — the domain model and the core error types — never in
//! adapter types. The persistence layer (and the test fakes) implement
//! them; the dependency points inward, from adapter to core.
//!
//! The ports are grouped by domain: `user`, `group`, and `health`.

pub mod group;
pub mod health;
pub mod user;

pub use group::{GroupCache, GroupRepository};
pub use health::{ComponentStatus, HealthCheck, HealthStatus};
pub use user::{UserCache, UserRepository};
