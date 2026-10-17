//! API version 1 — the versioned scope of the public HTTP API.
//!
//! Everything here is v1-specific: the route table (`/api/v1/...`), the
//! handlers, and the request/response DTOs. Version-independent
//! infrastructure (the error envelope, i18n, metrics, OpenAPI document)
//! lives at the `api` level.
//!
//! Adding a version 2 means adding a sibling `api/v2` module with its
//! own handlers and DTOs and registering both routers — v1 keeps
//! working untouched while v2 is developed and clients migrate.

pub mod dto;
pub mod handlers;
pub mod routes;

pub use routes::v1_router;
