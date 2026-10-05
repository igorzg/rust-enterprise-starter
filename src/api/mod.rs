//! Presentation layer: Axum router, HTTP handlers, request/response
//! mapping, OpenAPI/Swagger, and the HTTP error mapping.
//!
//! Handlers contain no SQL, cache logic, or business rules — they
//! decode requests, call the services, and encode responses.

pub mod dto;
pub mod error;
pub mod handlers;
pub mod i18n;
pub mod metrics;
pub mod openapi;
pub mod routes;
