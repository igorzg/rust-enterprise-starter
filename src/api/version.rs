//! Single source of truth for the public API version.
//!
//! Everything version-dependent reads from here: the router nests the
//! v1 endpoints under [`API_BASE`], every v1 response carries
//! [`API_VERSION_HEADER`], and the OpenAPI document reports
//! [`API_CONTRACT_VERSION`].
//!
//! The utoipa `#[utoipa::path(path = ...)]` and `info(version = ...)`
//! attributes require string literals, so the version appears twice in
//! the code (here, and in the OpenAPI attributes of `v1::handlers`).
//! `tests/api_versioning.rs` asserts the two agree — a drift fails the
//! build's test run instead of silently shipping a wrong contract.

/// The versioned segment of the API base path.
pub const API_VERSION: &str = "v1";

/// Base path of the v1 API (`/api/v1`), used to nest the v1 router.
///
/// `concat!` only accepts literals, so this is written out —
/// `tests/api_versioning.rs` asserts it equals `/api/{API_VERSION}`,
/// so the two cannot drift.
pub const API_BASE: &str = "/api/v1";

/// Response header that signals which API version served the response.
///
/// Canonical lowercase form — `HeaderName::from_static` requires it,
/// and header names are case-insensitive on the wire (RFC 9110 §5.1),
/// so the conventional `X-API-Version` spelling is satisfied too.
pub const API_VERSION_HEADER: &str = "x-api-version";

/// Semver of the v1 contract, reported as `info.version` in the OpenAPI
/// document. Independent of the crate version in `Cargo.toml` — the
/// contract is what clients integrate against.
pub const API_CONTRACT_VERSION: &str = "1.0.0";
