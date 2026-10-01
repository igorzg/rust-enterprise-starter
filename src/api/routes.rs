use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, header::HeaderValue};
use axum::routing::{get, post};
use tower_http::compression::CompressionLayer;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::{DefaultOnRequest, TraceLayer};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::api::error::ApiError;
use crate::api::handlers;
use crate::api::i18n;
use crate::api::metrics;
use crate::api::openapi::ApiDoc;
use crate::state::AppState;

/// Maximum accepted request body size (1 MiB).
const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Build the application router: health endpoints, the `/api/v1` user
/// API, OpenAPI + Swagger UI, CORS, body limits, gzip, tracing, and a
/// JSON 404 fallback.
pub fn build_router(state: AppState, cors_origins: Vec<String>) -> Router {
    let api_v1 = Router::new()
        .route("/users", post(handlers::user_handler::create_user))
        .route(
            "/users/{id}",
            get(handlers::user_handler::get_user).delete(handlers::user_handler::delete_user),
        )
        .layer(RequestBodyLimitLayer::new(MAX_BODY_BYTES))
        .layer(CompressionLayer::new())
        .with_state(state.clone());

    // The Swagger UI crate serves the OpenAPI document at the `.url` path
    // and the interactive UI under the `.new` path.
    let swagger_ui = SwaggerUi::new("/swagger-ui").url("/openapi.json", ApiDoc::openapi());

    Router::new()
        .route("/metrics", get(metrics::metrics_handler))
        .route("/health", get(handlers::health::health))
        .route("/ready", get(handlers::health::ready))
        .nest("/api/v1", api_v1)
        .merge(swagger_ui)
        // The fallback is registered before the metrics layer so
        // unmatched-route 404s are counted too, under the bounded
        // `<unmatched>` path label.
        .fallback(not_found)
        .layer(axum::middleware::from_fn(metrics::track_http_metrics))
        .layer(build_cors(cors_origins))
        .layer(axum::middleware::from_fn(i18n::locale_middleware))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &Request<Body>| {
                    tracing::span!(
                        tracing::Level::INFO,
                        "http_request",
                        method = %request.method(),
                        uri = %request.uri()
                    )
                })
                .on_request(DefaultOnRequest::new().level(tracing::Level::INFO)),
        )
        .with_state(state)
}

async fn not_found() -> ApiError {
    ApiError::RouteNotFound
}

/// CORS from `CORS_ALLOWED_ORIGINS`:
/// - empty  → CORS disabled (same-origin only);
/// - `*`    → explicit any-origin (only when the operator asks for it);
/// - list   → exactly the listed origins.
fn build_cors(origins: Vec<String>) -> CorsLayer {
    if origins.is_empty() {
        return CorsLayer::new();
    }
    if origins.iter().any(|origin| origin == "*") {
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
    }
    let allowed = origins
        .iter()
        .filter_map(|origin| origin.parse::<HeaderValue>().ok())
        .collect::<Vec<_>>();
    CorsLayer::new()
        .allow_origin(allowed)
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any)
}
