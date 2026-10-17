//! The v1 route table: all endpoints under the `/api/v1` base path,
//! plus the layers that apply to every v1 request — body size limit,
//! gzip, and the `X-API-Version` response header.

use axum::Router;
use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{delete, get, post};
use tower_http::compression::CompressionLayer;
use tower_http::limit::RequestBodyLimitLayer;

use crate::api::v1::handlers;
use crate::api::version;
use crate::state::AppState;

/// Maximum accepted request body size (1 MiB).
const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Build the v1 router, to be nested under [`version::API_BASE`].
///
/// The router stays stateful (`Router<AppState>`); the outer router in
/// [`crate::api::routes::build_router`] applies `.with_state(state)` to
/// the whole tree, so handlers resolve `State<AppState>` as usual.
///
/// Every response produced inside this scope — success, validation
/// error, or not-found — carries `X-API-Version: v1`. Requests that
/// match no v1 route (e.g. `/api/v1/nope`) fall through to the
/// app-level 404 fallback, which does not carry the header.
pub fn v1_router() -> Router<AppState> {
    Router::new()
        .route("/users", post(handlers::user_handler::create_user))
        .route(
            "/users/{id}",
            get(handlers::user_handler::get_user).delete(handlers::user_handler::delete_user),
        )
        .route(
            "/users/{id}/groups",
            get(handlers::group_handler::user_groups),
        )
        .route(
            "/groups",
            get(handlers::group_handler::list_groups).post(handlers::group_handler::create_group),
        )
        .route(
            "/groups/{id}",
            get(handlers::group_handler::get_group).delete(handlers::group_handler::delete_group),
        )
        .route(
            "/groups/{id}/users",
            post(handlers::group_handler::add_group_member),
        )
        .route(
            "/groups/{id}/users/{userId}",
            delete(handlers::group_handler::remove_group_member),
        )
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(CompressionLayer::new())
        .layer(axum::middleware::from_fn(version_header))
}

/// Stamp every v1 response with `X-API-Version: v1`.
///
/// Runs inside the v1 scope only, so app-level surfaces (`/health`,
/// `/metrics`, OpenAPI, the 404 fallback) never carry the header.
async fn version_header(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    response.headers_mut().insert(
        HeaderName::from_static(version::API_VERSION_HEADER),
        HeaderValue::from_static(version::API_VERSION),
    );
    response
}
