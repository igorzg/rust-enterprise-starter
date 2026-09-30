//! Persistence adapters: the PostgreSQL (SQLx) and Redis implementations
//! of the core's output ports, the persistence entity, and the readiness
//! probes. The dependency points inward — this layer implements the
//! ports owned by `services` and maps its entity to the core's domain
//! model at the edge.
//!
//! Database and cache specifics never leak into the core or the
//! presentation layer.

pub mod cache;
pub mod database;
pub mod health;
