//! Per-request locale resolution for i18n.
//!
//! The core never sees a locale: this module resolves the request locale
//! from `Accept-Language`, stores it in a `tokio::task_local!` for the
//! duration of the request, and the error mapping reads it when rendering
//! localized messages.

use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

tokio::task_local! {
    static LOCALE: String;
}

/// Resolve the request locale from an `Accept-Language` header value,
/// preferring exact matches then language subtags, falling back to `en`.
pub fn resolve_locale(accept_language: Option<&str>) -> String {
    let mut candidates = accept_language
        .map(parse_accept_language)
        .unwrap_or_default();

    let available = rust_i18n::available_locales!();

    for candidate in candidates.drain(..) {
        let tag = candidate.as_str();
        if available.iter().any(|locale| locale.as_ref() == tag) {
            return candidate;
        }
        if let Some((language, _)) = candidate.split_once('-') {
            if available.iter().any(|locale| locale.as_ref() == language) {
                return language.to_string();
            }
        }
    }

    "en".to_string()
}

/// Middleware that resolves and scopes the request locale so error
/// responses are rendered in the client's language.
pub async fn locale_middleware(req: Request, next: Next) -> Response {
    let locale = resolve_locale(
        req.headers()
            .get(axum::http::header::ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok()),
    );
    LOCALE.scope(locale, next.run(req)).await
}

/// The locale of the current request, defaulting to `en` outside a request.
pub fn current_locale() -> String {
    LOCALE
        .try_with(Clone::clone)
        .unwrap_or_else(|_| "en".to_owned())
}

/// Parse an `Accept-Language` header into locale tags ordered by quality.
fn parse_accept_language(header: &str) -> Vec<String> {
    let mut weighted: Vec<(f32, String)> = header
        .split(',')
        .filter_map(|part| {
            let mut segments = part.trim().split(';');
            let tag = segments.next()?.trim().to_string();
            let quality = segments
                .next()
                .and_then(|segment| segment.trim().strip_prefix("q="))
                .and_then(|value| value.parse::<f32>().ok())
                .unwrap_or(1.0);
            Some((quality, tag))
        })
        .collect();

    weighted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    weighted.into_iter().map(|(_, tag)| tag).collect()
}
