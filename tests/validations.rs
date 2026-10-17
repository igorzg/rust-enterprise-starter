//! Direct tests for the API-layer validation functions
//! (`src/api/validations.rs`). These pin down per-field rules and
//! normalization; the end-to-end behavior over HTTP is covered by
//! `users_api.rs` and `groups_api.rs`.

use rstarter::api::validations::{validate_email, validate_group_name, validate_user_name};
use rstarter::services::errors::{AppError, ValidationError};

#[test]
fn validate_user_name_trims_and_accepts() {
    assert_eq!(validate_user_name("  Jane Doe  ").unwrap(), "Jane Doe");
}

#[test]
fn validate_user_name_rejects_blank() {
    assert!(matches!(
        validate_user_name("   ").unwrap_err(),
        AppError::Validation(ValidationError::NameLength { .. })
    ));
}

#[test]
fn validate_user_name_rejects_over_max_length() {
    let name = "a".repeat(121);
    assert!(matches!(
        validate_user_name(&name).unwrap_err(),
        AppError::Validation(ValidationError::NameLength { max: 120 })
    ));
}

#[test]
fn validate_user_name_accepts_max_length() {
    let name = "a".repeat(120);
    assert_eq!(validate_user_name(&name).unwrap(), name);
}

#[test]
fn validate_email_trims_and_lowercases() {
    assert_eq!(
        validate_email("  JANE@Example.COM  ").unwrap(),
        "jane@example.com"
    );
}

#[test]
fn validate_email_rejects_blank() {
    assert!(matches!(
        validate_email("   ").unwrap_err(),
        AppError::Validation(ValidationError::EmailLength { max: 254 })
    ));
}

#[test]
fn validate_email_rejects_invalid_formats() {
    for email in [
        "no-at-sign.com",
        "double@@example.com",
        "@example.com",
        "jane@",
        "jane@example",
        "jane@.example.com",
        "jane@example.com.",
        "jane@example..com",
    ] {
        assert!(
            validate_email(email).is_err(),
            "expected rejection for {email:?}"
        );
    }
}

#[test]
fn validate_email_rejects_over_max_length() {
    // 254 is the RFC 5321 limit; 243 + "@example.com" (12) = 255.
    let email = format!("{}@example.com", "a".repeat(243));
    assert!(matches!(
        validate_email(&email).unwrap_err(),
        AppError::Validation(ValidationError::EmailLength { max: 254 })
    ));
}

#[test]
fn validate_group_name_trims_and_accepts() {
    assert_eq!(
        validate_group_name("  administrators  ").unwrap(),
        "administrators"
    );
}

#[test]
fn validate_group_name_rejects_blank() {
    assert!(matches!(
        validate_group_name("").unwrap_err(),
        AppError::Validation(ValidationError::GroupNameLength { .. })
    ));
}

#[test]
fn validate_group_name_rejects_over_max_length() {
    let name = "a".repeat(121);
    assert!(matches!(
        validate_group_name(&name).unwrap_err(),
        AppError::Validation(ValidationError::GroupNameLength { max: 120 })
    ));
}
