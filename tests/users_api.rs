//! API tests: exercise the full Axum router with in-memory fakes —
//! no PostgreSQL or Redis required.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt; // for `oneshot`
use uuid::Uuid;

mod common;

use common::test_state;
use rstarter::api::routes::build_router;

/// A valid UUID that no test seeds — used for "not found" requests.
const UNKNOWN_ID: &str = "00000000-0000-0000-0000-000000000000";

/// A router backed by one shared in-memory `AppState`, so multiple
/// requests within a single test observe each other's effects.
struct TestApp {
    router: Router,
}

impl TestApp {
    fn new(healthy: bool) -> Self {
        let (state, _, _) = test_state(healthy);
        Self {
            router: build_router(state, Vec::new()),
        }
    }

    async fn call(&self, request: Request<Body>) -> (StatusCode, Value) {
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let value = if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).unwrap_or(Value::Null)
        };
        (status, value)
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

#[tokio::test]
async fn health_returns_ok_without_checking_dependencies() {
    let app = TestApp::new(false);

    let (status, body) = app.call(get("/health")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn ready_reports_dependencies() {
    let healthy = TestApp::new(true);
    let (status, body) = healthy.call(get("/ready")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ready");
    assert_eq!(body["components"][0]["name"], "postgres");
    assert_eq!(body["components"][0]["up"], true);
    assert_eq!(body["components"][1]["name"], "redis");
    assert_eq!(body["components"][1]["up"], true);

    let unhealthy = TestApp::new(false);
    let (status, body) = unhealthy.call(get("/ready")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["status"], "unavailable");
    assert_eq!(body["components"][0]["up"], false);
    assert_eq!(body["components"][1]["up"], false);
}

#[tokio::test]
async fn create_user_returns_201_with_id() {
    let app = TestApp::new(true);

    let (status, body) = app
        .call(post_json(
            "/api/v1/users",
            &json!({ "name": "Jane Doe", "email": "jane@example.com" }),
        ))
        .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(Uuid::parse_str(body["id"].as_str().unwrap()).is_ok());
    assert_eq!(body["name"], "Jane Doe");
    assert_eq!(body["email"], "jane@example.com");
}

#[tokio::test]
async fn create_user_with_invalid_email_returns_400() {
    let app = TestApp::new(true);

    let (status, body) = app
        .call(post_json(
            "/api/v1/users",
            &json!({ "name": "Jane", "email": "not-an-email" }),
        ))
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn create_user_rejects_invalid_emails() {
    let app = TestApp::new(true);

    for email in [
        "",
        "no-at-sign.com",
        "double@@example.com",
        "@example.com",
        "jane@",
        "jane@example",
        "jane@.example.com",
        "jane@example.com.",
        "jane@example..com",
    ] {
        let (status, body) = app
            .call(post_json(
                "/api/v1/users",
                &json!({ "name": "Jane", "email": email }),
            ))
            .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "expected 400 for {email:?}"
        );
        assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
    }
}

#[tokio::test]
async fn create_user_rejects_blank_name() {
    let app = TestApp::new(true);

    let (status, body) = app
        .call(post_json(
            "/api/v1/users",
            &json!({ "name": "   ", "email": "jane@example.com" }),
        ))
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn create_user_normalizes_email_case_and_whitespace() {
    let app = TestApp::new(true);

    let (status, body) = app
        .call(post_json(
            "/api/v1/users",
            &json!({ "name": "Jane", "email": "  JANE@Example.COM  " }),
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["email"], "jane@example.com");

    // Normalization feeds the uniqueness business rule: the same email
    // in canonical form is a conflict.
    let (status, body) = app
        .call(post_json(
            "/api/v1/users",
            &json!({ "name": "Jane Copy", "email": "jane@example.com" }),
        ))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "CONFLICT");
}

#[tokio::test]
async fn create_user_with_malformed_json_returns_400() {
    let app = TestApp::new(true);

    let mut request = Request::builder()
        .method("POST")
        .uri("/api/v1/users")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    *request.body_mut() = Body::from("{not json");

    let (status, body) = app.call(request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn duplicate_email_returns_409() {
    let app = TestApp::new(true);
    let body = json!({ "name": "Jane", "email": "jane@example.com" });

    let (status, _) = app.call(post_json("/api/v1/users", &body)).await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = app.call(post_json("/api/v1/users", &body)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "CONFLICT");
}

#[tokio::test]
async fn get_user_after_create() {
    let app = TestApp::new(true);

    let (status, created) = app
        .call(post_json(
            "/api/v1/users",
            &json!({ "name": "Jane", "email": "jane@example.com" }),
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();

    let (status, body) = app.call(get(&format!("/api/v1/users/{id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], id);
    assert_eq!(body["email"], "jane@example.com");
}

#[tokio::test]
async fn get_missing_user_returns_404_envelope() {
    let app = TestApp::new(true);

    let (status, body) = app.call(get(&format!("/api/v1/users/{UNKNOWN_ID}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn invalid_id_returns_400() {
    let app = TestApp::new(true);

    let (status, body) = app.call(get("/api/v1/users/abc")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn delete_user_returns_204_then_404() {
    let app = TestApp::new(true);

    let (status, created) = app
        .call(post_json(
            "/api/v1/users",
            &json!({ "name": "Jane", "email": "jane@example.com" }),
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = created["id"].as_str().unwrap();

    let (status, _) = app.call(delete(&format!("/api/v1/users/{id}"))).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = app.call(get(&format!("/api/v1/users/{id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn unknown_route_returns_404_json_envelope() {
    let app = TestApp::new(true);

    let (status, body) = app.call(get("/definitely/not/a/route")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn database_error_returns_sanitized_500_envelope() {
    let router = build_router(common::failing_state(), Vec::new());

    let response = router
        .oneshot(get(&format!("/api/v1/users/{UNKNOWN_ID}")))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(value["error"]["code"], "DATABASE_ERROR");
    assert_eq!(value["error"]["message"], "an internal error occurred");

    // The internal detail never leaks into the response body.
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        !text.contains("on fire"),
        "5xx body must stay sanitized: {text}"
    );
}

#[tokio::test]
async fn metrics_endpoint_exposes_prometheus_text_with_normalized_paths() {
    let (state, _, _) = test_state(true);
    let router = build_router(state, Vec::new());

    // Exercise a real route so `http_requests_total` has a sample.
    let _ = router
        .clone()
        .oneshot(get(&format!("/api/v1/users/{UNKNOWN_ID}")))
        .await
        .unwrap();

    // An unmatched route must be counted too, under the bounded label.
    let _ = router
        .clone()
        .oneshot(get("/definitely/not/a/route"))
        .await
        .unwrap();

    let response = router.oneshot(get("/metrics")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap(),
        "text/plain; version=0.0.4; charset=utf-8"
    );

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();

    // The id is normalized to the route template, never emitted raw.
    assert!(text.contains("http_requests_total"), "metrics: {text}");
    assert!(text.contains("users/{id}"), "metrics: {text}");
    assert!(!text.contains(UNKNOWN_ID), "metrics: {text}");

    // Unmatched routes are counted, but never under the raw path.
    assert!(text.contains("<unmatched>"), "metrics: {text}");
    assert!(!text.contains("definitely/not/a/route"), "metrics: {text}");
}
