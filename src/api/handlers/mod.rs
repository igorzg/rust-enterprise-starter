//! App-level (version-independent) handlers: the health and readiness
//! probes. Versioned handlers live in [`crate::api::v1::handlers`].

pub mod health;
