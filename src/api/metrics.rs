//! Prometheus metrics: recorder setup, the `/metrics` handler, and the
//! RED (rate/errors/duration) HTTP middleware.
//!
//! Recording is driven by the [`metrics`] facade; the exporter renders the
//! snapshot in the Prometheus text format. The core (`services`) stays
//! metrics-free: HTTP metrics are recorded here in the API layer, and
//! database/cache metrics in the persistence adapters. Add more
//! instrumentation with `metrics::counter!` / `metrics::histogram!` /
//! `metrics::gauge!` anywhere outside the core.

use std::sync::OnceLock;
use std::time::Instant;

use axum::extract::{MatchedPath, Request};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

/// `path` label for requests matching no route (the JSON 404 fallback).
/// A constant on purpose: the raw path would be unbounded and let error
/// probes inflate label cardinality.
const UNMATCHED_PATH: &str = "<unmatched>";

/// The recorder handle, installed exactly once per process.
static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Install the Prometheus recorder as the global [`metrics`] recorder.
/// Idempotent: only the first call has an effect. Called once at startup
/// (and once per test process).
pub fn init() {
    HANDLE.get_or_init(|| {
        PrometheusBuilder::new()
            .install_recorder()
            .expect("failed to install the Prometheus recorder")
    });
}

/// Serve the current metrics snapshot in the Prometheus text format.
pub async fn metrics_handler() -> impl IntoResponse {
    let handle = HANDLE
        .get()
        .expect("metrics recorder not installed; call api::metrics::init at startup");
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        handle.render(),
    )
}

/// Axum middleware recording RED metrics. Applied with `layer` (not
/// `route_layer`) so the 404 fallback is counted too. The route template
/// (not the raw path) is used as the `path` label — this keeps label
/// cardinality bounded as ids in the URL change; requests matching no
/// route fall back to [`UNMATCHED_PATH`].
pub async fn track_http_metrics(req: Request, next: Next) -> Response {
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|matched| matched.as_str().to_owned())
        .unwrap_or_else(|| UNMATCHED_PATH.to_owned());

    // The scrape itself is not application traffic; do not count it.
    if path == "/metrics" {
        return next.run(req).await;
    }

    let started = Instant::now();
    let method = req.method().to_string();

    let in_flight = metrics::gauge!(
        "http_requests_in_flight",
        "method" => method.clone(),
        "path" => path.clone(),
    );
    in_flight.increment(1.0);

    let response = next.run(req).await;

    let status = response.status().as_u16().to_string();
    metrics::counter!(
        "http_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status.clone(),
    )
    .increment(1);
    metrics::histogram!(
        "http_request_duration_seconds",
        "method" => method,
        "path" => path,
        "status" => status,
    )
    .record(started.elapsed().as_secs_f64());

    in_flight.decrement(1.0);

    response
}
