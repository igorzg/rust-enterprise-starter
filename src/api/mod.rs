//! Presentation layer: Axum router, HTTP handlers, request/response
//! mapping, OpenAPI/Swagger, and the HTTP error mapping.
//!
//! Versioned endpoints live in [`v1`]; the app-level router
//! ([`routes`]) hosts the version-independent surface — health and
//! readiness probes, metrics, Swagger UI — and nests each version
//! under its base path (see [`version`]).
//!
//! Handlers contain no SQL, cache logic, or business rules — they
//! decode requests, call the services, and encode responses.

pub mod error;
pub mod handlers;
pub mod i18n;
pub mod metrics;
pub mod openapi;
pub mod routes;
pub mod v1;
pub mod version;
