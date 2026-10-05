//! Config parsing: `CORS_ALLOWED_ORIGINS` fails fast on invalid entries
//! instead of silently dropping them at route build time.

use rstarter::config::parse_cors_origins;

#[test]
fn cors_origins_empty_disables_cors() {
    assert!(parse_cors_origins("").unwrap().is_empty());
}

#[test]
fn cors_origins_trims_entries_and_ignores_trailing_comma() {
    assert_eq!(
        parse_cors_origins("https://a.example, https://b.example ,").unwrap(),
        vec!["https://a.example", "https://b.example"]
    );
}

#[test]
fn cors_origins_wildcard_is_valid() {
    assert_eq!(parse_cors_origins("*").unwrap(), vec!["*"]);
}

#[test]
fn cors_origins_rejects_invalid_entry() {
    for raw in [
        "not an origin",
        "https://ok.example/path",
        "ftp://ok.example",
        "example.com",
    ] {
        let error = parse_cors_origins(raw).unwrap_err();
        assert!(
            error.to_string().contains("CORS_ALLOWED_ORIGINS"),
            "expected rejection for {raw:?}: {error}"
        );
    }
}
