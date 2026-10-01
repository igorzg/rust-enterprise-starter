//! 5xx error logging: the full source chain (e.g. the simulated SQLx
//! error behind `AppError::Database`) must reach the server log, while
//! the HTTP envelope stays sanitized.

use std::io::Write;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt; // for `oneshot`

mod common;

use common::failing_state;
use rstarter::api::routes::build_router;

/// A valid UUID that no repository seeds — used for error requests.
const UNKNOWN_ID: &str = "00000000-0000-0000-0000-000000000000";

/// Writes formatted log output into a shared buffer.
#[derive(Clone)]
struct SharedWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedWriter {
    type Writer = SharedWriter;

    fn make_writer(&'a self) -> Self::Writer {
        Self {
            buffer: self.buffer.clone(),
        }
    }
}

#[tokio::test]
async fn database_error_logs_the_full_source_chain() {
    let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

    // This test binary runs as its own process; install the subscriber
    // before triggering the 500 so the event lands in our buffer.
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(SharedWriter {
            buffer: buffer.clone(),
        })
        .try_init()
        .expect("install test subscriber");

    let router = build_router(failing_state(), Vec::new());
    let response = router
        .oneshot(get(&format!("/api/v1/users/{UNKNOWN_ID}")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let _ = response.into_body().collect().await;

    let log = String::from_utf8(buffer.lock().unwrap().to_vec()).unwrap();

    // Both the outer error and the inner (source) detail reach the log.
    assert!(log.contains("request failed"), "log: {log}");
    assert!(log.contains("database error"), "log: {log}");
    assert!(
        log.contains("on fire"),
        "inner detail missing from log: {log}"
    );
}

fn get(path: &str) -> Request<Body> {
    Request::builder().uri(path).body(Body::empty()).unwrap()
}
