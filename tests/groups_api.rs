//! API tests for the group endpoints: exercise the full Axum router with
//! in-memory fakes — no PostgreSQL or Redis required.

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

/// Create a user through the API and return its id.
async fn create_user(app: &TestApp) -> String {
    let (status, body) = app
        .call(post_json(
            "/api/v1/users",
            &json!({ "name": "Jane Doe", "email": "jane@example.com" }),
        ))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    body["id"].as_str().unwrap().to_string()
}

/// Create a group through the API and return its id.
async fn create_group(app: &TestApp, name: &str) -> String {
    let (status, body) = app
        .call(post_json("/api/v1/groups", &json!({ "name": name })))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    body["id"].as_str().unwrap().to_string()
}

/// Assign a user to a group through the API.
async fn assign(app: &TestApp, group_id: &str, user_id: &str) -> StatusCode {
    let (status, _) = app
        .call(post_json(
            &format!("/api/v1/groups/{group_id}/users"),
            &json!({ "user_id": user_id }),
        ))
        .await;
    status
}

#[tokio::test]
async fn create_group_returns_201_with_id() {
    let app = TestApp::new(true);

    let (status, body) = app
        .call(post_json("/api/v1/groups", &json!({ "name": "admins" })))
        .await;

    assert_eq!(status, StatusCode::CREATED);
    assert!(Uuid::parse_str(body["id"].as_str().unwrap()).is_ok());
    assert_eq!(body["name"], "admins");
}

#[tokio::test]
async fn create_group_rejects_blank_name() {
    let app = TestApp::new(true);

    let (status, body) = app
        .call(post_json("/api/v1/groups", &json!({ "name": "   " })))
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn create_group_rejects_overlong_name() {
    let app = TestApp::new(true);

    let name = "a".repeat(121);
    let (status, body) = app
        .call(post_json("/api/v1/groups", &json!({ "name": name })))
        .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn create_group_with_malformed_json_returns_400() {
    let app = TestApp::new(true);

    let mut request = Request::builder()
        .method("POST")
        .uri("/api/v1/groups")
        .header("content-type", "application/json")
        .body(Body::empty())
        .unwrap();
    *request.body_mut() = Body::from("{not json");

    let (status, body) = app.call(request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn duplicate_group_name_returns_409() {
    let app = TestApp::new(true);

    let (status, _) = app
        .call(post_json("/api/v1/groups", &json!({ "name": "admins" })))
        .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = app
        .call(post_json("/api/v1/groups", &json!({ "name": "admins" })))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "CONFLICT");
}

#[tokio::test]
async fn list_groups_returns_created_groups() {
    let app = TestApp::new(true);
    create_group(&app, "admins").await;
    create_group(&app, "editors").await;

    let (status, body) = app.call(get("/api/v1/groups")).await;
    assert_eq!(status, StatusCode::OK);

    let mut names: Vec<String> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|group| group["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(names, vec!["admins", "editors"]);
}

#[tokio::test]
async fn get_group_returns_detail_with_members() {
    let app = TestApp::new(true);
    let user_id = create_user(&app).await;
    let group_id = create_group(&app, "admins").await;
    assert_eq!(
        assign(&app, &group_id, &user_id).await,
        StatusCode::NO_CONTENT
    );

    let (status, body) = app.call(get(&format!("/api/v1/groups/{group_id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], group_id);
    assert_eq!(body["name"], "admins");
    assert_eq!(body["members"].as_array().unwrap().len(), 1);
    assert_eq!(body["members"][0]["id"], user_id);
}

#[tokio::test]
async fn get_missing_group_returns_404_envelope() {
    let app = TestApp::new(true);

    let (status, body) = app.call(get(&format!("/api/v1/groups/{UNKNOWN_ID}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn invalid_group_id_returns_400() {
    let app = TestApp::new(true);

    let (status, body) = app.call(get("/api/v1/groups/abc")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "VALIDATION_ERROR");
}

#[tokio::test]
async fn delete_group_returns_204_then_404() {
    let app = TestApp::new(true);
    let group_id = create_group(&app, "admins").await;

    let (status, _) = app
        .call(delete(&format!("/api/v1/groups/{group_id}")))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body) = app.call(get(&format!("/api/v1/groups/{group_id}"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn add_group_member_requires_existing_group_and_user() {
    let app = TestApp::new(true);
    let user_id = create_user(&app).await;

    // Unknown group.
    let (status, body) = app
        .call(post_json(
            &format!("/api/v1/groups/{UNKNOWN_ID}/users"),
            &json!({ "user_id": user_id }),
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");

    // Unknown user.
    let group_id = create_group(&app, "admins").await;
    let (status, body) = app
        .call(post_json(
            &format!("/api/v1/groups/{group_id}/users"),
            &json!({ "user_id": UNKNOWN_ID }),
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn add_group_member_is_idempotent() {
    let app = TestApp::new(true);
    let user_id = create_user(&app).await;
    let group_id = create_group(&app, "admins").await;

    for _ in 0..2 {
        assert_eq!(
            assign(&app, &group_id, &user_id).await,
            StatusCode::NO_CONTENT
        );
    }

    // Still exactly one membership.
    let (status, body) = app.call(get(&format!("/api/v1/groups/{group_id}"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["members"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn remove_group_member_returns_204_then_404() {
    let app = TestApp::new(true);
    let user_id = create_user(&app).await;
    let group_id = create_group(&app, "admins").await;
    assert_eq!(
        assign(&app, &group_id, &user_id).await,
        StatusCode::NO_CONTENT
    );

    let (status, _) = app
        .call(delete(&format!(
            "/api/v1/groups/{group_id}/users/{user_id}"
        )))
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // Removing a non-membership is a 404.
    let (status, body) = app
        .call(delete(&format!(
            "/api/v1/groups/{group_id}/users/{user_id}"
        )))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn user_groups_lists_memberships() {
    let app = TestApp::new(true);
    let user_id = create_user(&app).await;
    let first = create_group(&app, "admins").await;
    let second = create_group(&app, "editors").await;
    for group_id in [&first, &second] {
        assert_eq!(
            assign(&app, group_id, &user_id).await,
            StatusCode::NO_CONTENT
        );
    }

    let (status, body) = app
        .call(get(&format!("/api/v1/users/{user_id}/groups")))
        .await;
    assert_eq!(status, StatusCode::OK);

    let mut names: Vec<String> = body
        .as_array()
        .unwrap()
        .iter()
        .map(|group| group["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(names, vec!["admins", "editors"]);
}

#[tokio::test]
async fn user_groups_requires_existing_user() {
    let app = TestApp::new(true);

    let (status, body) = app
        .call(get(&format!("/api/v1/users/{UNKNOWN_ID}/groups")))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn database_error_returns_sanitized_500_envelope() {
    let router = build_router(common::failing_state(), Vec::new());

    let response = router
        .oneshot(get(&format!("/api/v1/groups/{UNKNOWN_ID}")))
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
