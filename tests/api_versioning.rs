//! API versioning contract tests.
//!
//! What this suite pins down:
//! - every v1 response — success, validation error, or not-found —
//!   carries `X-API-Version: v1`;
//! - app-level surfaces (health probes, metrics, the 404 fallback)
//!   never carry the header — including unknown paths under `/api/v1`,
//!   which fall through to the app fallback because the v1 router keeps
//!   the default fallback;
//! - the OpenAPI document and the base path stay in sync with
//!   `rstarter::api::version` (the utoipa attributes take string
//!   literals only, so the sync is enforced here, not at compile time).

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt; // for `oneshot`
use utoipa::OpenApi;

mod common;

use common::test_state;
use rstarter::api::openapi::ApiDoc;
use rstarter::api::routes::build_router;
use rstarter::api::version::{API_BASE, API_CONTRACT_VERSION, API_VERSION, API_VERSION_HEADER};

/// A valid UUID that no test seeds — used for "not found" requests.
const UNKNOWN_ID: &str = "00000000-0000-0000-0000-000000000000";

/// A router backed by in-memory fakes. Every request yields the status,
/// the `X-API-Version` header value (if present), and the JSON body.
struct TestApp {
    router: Router,
}

impl TestApp {
    fn new() -> Self {
        let (state, _, _) = test_state(true);
        Self {
            router: build_router(state, Vec::new()),
        }
    }

    async fn call(&self, request: Request<Body>) -> (StatusCode, Option<String>, Value) {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let version = response
            .headers()
            .get(API_VERSION_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let value = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).unwrap_or(Value::Null)
        };
        (status, version, value)
    }
}

fn get(path: &str) -> Request<Body> {
    Request::builder().uri(path).body(Body::empty()).unwrap()
}

fn delete(path: &str) -> Request<Body> {
    Request::builder()
        .method("DELETE")
        .uri(path)
        .body(Body::empty())
        .unwrap()
}

fn post_json(path: &str, body: &Value) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    *request.body_mut() = Body::from(body.to_string());
    request
}

fn malformed_json_post(path: &str) -> Request<Body> {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    *request.body_mut() = Body::from("{not json");
    request
}

#[tokio::test]
async fn v1_success_responses_carry_version_header() {
    let app = TestApp::new();

    let (status, version, created) = app
        .call(post_json(
            "/api/v1/users",
            &json!({ "name": "Jane", "email": "jane@example.com" }),
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(version.as_deref(), Some(API_VERSION));
    let id = created["id"].as_str().unwrap();

    let (status, version, _) = app.call(get(&format!("/api/v1/users/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(version.as_deref(), Some(API_VERSION));

    let (status, version, _) = app.call(delete(&format!("/api/v1/users/{id}"))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(version.as_deref(), Some(API_VERSION));

    let (status, version, _) = app
        .call(post_json("/api/v1/groups", &json!({ "name": "eng" })))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(version.as_deref(), Some(API_VERSION));

    let (status, version, _) = app.call(get("/api/v1/groups")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(version.as_deref(), Some(API_VERSION));
}

#[tokio::test]
async fn v1_error_responses_carry_version_header() {
    let app = TestApp::new();

    let (status, version, _) = app.call(get(&format!("/api/v1/users/{UNKNOWN_ID}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(version.as_deref(), Some(API_VERSION));

    let (status, version, _) = app.call(get("/api/v1/users/abc")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(version.as_deref(), Some(API_VERSION));

    let (status, version, _) = app.call(malformed_json_post("/api/v1/users")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(version.as_deref(), Some(API_VERSION));
}

#[tokio::test]
async fn app_level_surfaces_carry_no_version_header() {
    let app = TestApp::new();

    // Probes and metrics are version-independent.
    for path in ["/health", "/ready", "/metrics"] {
        let (status, version, _) = app.call(get(path)).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert_eq!(version, None, "{path}");
    }

    // Unversioned paths and unknown API versions hit the app-level
    // 404 fallback — including unknown subpaths of `/api/v1`, which
    // fall through because the v1 router keeps its default fallback.
    for path in [
        "/users",
        "/api/users",
        "/api/v2/users",
        "/api/v1/nope",
        "/definitely/not/a/route",
    ] {
        let (status, version, _) = app.call(get(path)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(version, None, "{path}");
    }
}

#[test]
fn openapi_doc_stays_in_sync_with_version_module() {
    let openapi = ApiDoc::openapi();

    // The utoipa attributes take string literals only, so the contract
    // version and the base path are guarded here instead of at compile
    // time.
    assert_eq!(openapi.info.version, API_CONTRACT_VERSION);
    assert_eq!(API_BASE, format!("/api/{API_VERSION}"));

    assert!(!openapi.paths.paths.is_empty(), "OpenAPI doc has no paths");
    let prefix = format!("{API_BASE}/");
    for path in openapi.paths.paths.keys() {
        assert!(
            path.starts_with(&prefix),
            "unversioned path in the OpenAPI doc: {path}"
        );
    }
}
