//! rstarter — a production-ready Rust REST API starter.
//!
//! The crate follows hexagonal architecture: a core surrounded by
//! adapters, with every dependency pointing inward at the core.
//!
//! - [`services`]: the core — the user domain model and commands, the
//!   output port traits, the error model, and the use-case services
//!   (business rules only). It never depends on `api` or `persistence`
//!   — nor on SQLx, Redis, or Axum.
//! - [`api`]: the input adapter — Axum routes, handlers, request/response
//!   DTOs, OpenAPI, and HTTP error mapping.
//! - [`persistence`]: the output adapters — the PostgreSQL (SQLx) and
//!   Redis implementations of the core's ports, the persistence entity,
//!   and the readiness probes.
//!
//! [`state`] is the composition root that wires the adapters to the
//! core's ports.

#[macro_use]
extern crate rust_i18n;

rust_i18n::i18n!("locales", fallback = "en");

pub mod api;
pub mod config;
pub mod persistence;
pub mod services;
pub mod state;
