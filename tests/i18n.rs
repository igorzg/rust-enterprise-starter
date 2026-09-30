//! i18n tests: locale resolution from `Accept-Language` and localized
//! error messages in the full router.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

mod common;

use common::test_state;
use rstarter::api::i18n::resolve_locale;
use rstarter::api::routes::build_router;

/// A valid UUID that no test seeds — used for "not found" requests.
const UNKNOWN_ID: &str = "00000000-0000-0000-0000-000000000000";

#[test]
fn locale_falls_back_to_english_when_header_is_absent() {
    assert_eq!(resolve_locale(None), "en");
}

#[test]
fn locale_resolves_exact_match() {
    assert_eq!(resolve_locale(Some("de")), "de");
}

#[test]
fn locale_falls_back_to_language_subtag() {
    assert_eq!(resolve_locale(Some("de-DE,de;q=0.9,en;q=0.8")), "de");
}

#[test]
fn locale_falls_back_to_english_for_unknown_language() {
    assert_eq!(resolve_locale(Some("xx-YY")), "en");
}

#[test]
fn locale_prefers_highest_quality_tag() {
    assert_eq!(resolve_locale(Some("de;q=0.5,en;q=0.9")), "en");
}

/// Fetch the localized `error.message` for a missing user (404).
async fn missing_user_message(accept_language: Option<&str>) -> String {
    let (state, _, _) = test_state(true);
    let router = build_router(state, Vec::new());

    let mut builder = Request::builder().uri(format!("/api/v1/users/{UNKNOWN_ID}"));
    if let Some(language) = accept_language {
        builder = builder.header("accept-language", language);
    }
    let response = router
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&body).unwrap();
    value["error"]["message"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn errors_localize_to_german_from_accept_language() {
    let message = missing_user_message(Some("de-DE,de;q=0.9,en;q=0.8")).await;
    assert_eq!(
        message,
        format!("Benutzer mit ID {UNKNOWN_ID} wurde nicht gefunden")
    );
}

#[tokio::test]
async fn errors_default_to_english() {
    let message = missing_user_message(None).await;
    assert_eq!(message, format!("user with id {UNKNOWN_ID} was not found"));
}

#[tokio::test]
async fn error_code_is_language_independent() {
    let (state, _, _) = test_state(true);
    let router = build_router(state, Vec::new());

    let response = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/users/{UNKNOWN_ID}"))
                .header("accept-language", "de")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["error"]["code"], "NOT_FOUND");
}
