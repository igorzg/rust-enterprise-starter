use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::services::ports::ComponentStatus;
use crate::state::AppState;

/// Liveness probe: the process is up. Does not touch dependencies, so it
/// never returns non-2xx merely because the database is down.
pub async fn health() -> impl IntoResponse {
    Json(serde_json::json!({ "status": "ok" }))
}

#[derive(Debug, Serialize)]
struct ReadyResponse {
    status: &'static str,
    components: Vec<ComponentStatus>,
}

/// Readiness probe: checks PostgreSQL and Redis. Returns 200 when every
/// dependency is up, 503 otherwise — usable as a k8s readiness gate.
pub async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let report = state.health.check().await;
    let (status, label) = if report.ready() {
        (StatusCode::OK, "ready")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
    };
    (
        status,
        Json(ReadyResponse {
            status: label,
            components: report.components,
        }),
    )
}
