//! API data transfer objects (presentation-layer types).
//!
//! DTOs are the boundary between the HTTP API and the application
//! layer: requests are validated and mapped to commands, responses
//! are mapped from the domain model. They are separate types so the
//! public contract can evolve independently of the internal model.

pub mod requests;
pub mod responses;

pub use requests::{AssignGroupMemberRequest, CreateGroupRequest, CreateUserRequest};
pub use responses::{GroupDetailResponse, GroupResponse, UserResponse};
