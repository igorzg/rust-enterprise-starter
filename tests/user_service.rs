//! Unit tests for the UserService (business rules, cache-aside,
//! invalidation). Request validation is covered by the API tests.

use std::sync::atomic::Ordering;

use uuid::Uuid;

use rstarter::services::domain::{CreateUserCommand, User};
use rstarter::services::errors::AppError;
use rstarter::services::ports::{UserCache, UserRepository};

mod common;

use common::test_state;

fn user(id: Uuid, name: &str, email: &str) -> User {
    User {
        id,
        name: name.to_string(),
        email: email.to_string(),
    }
}

fn command(name: &str, email: &str) -> CreateUserCommand {
    CreateUserCommand {
        name: name.to_string(),
        email: email.to_string(),
    }
}

#[tokio::test]
async fn create_persists_user_and_populates_cache() {
    let (state, repository, cache) = test_state(true);

    let created = state
        .user_service
        .create(command("Jane Doe", "jane@example.com"))
        .await
        .unwrap();

    assert!(!created.id.is_nil());
    assert_eq!(created.name, "Jane Doe");
    assert_eq!(created.email, "jane@example.com");
    let stored = repository.find_by_id(created.id).await.unwrap().unwrap();
    assert_eq!(stored, created);
    let cached = cache.get(created.id).await.unwrap().unwrap();
    assert_eq!(cached, created);
}

#[tokio::test]
async fn create_rejects_duplicate_email() {
    let (state, _, _) = test_state(true);

    state
        .user_service
        .create(command("Jane", "jane@example.com"))
        .await
        .unwrap();

    let error = state
        .user_service
        .create(command("Jane Copy", "jane@example.com"))
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Conflict(_)));
}

#[tokio::test]
async fn create_trusts_prevalidated_command_fields() {
    let (state, repository, _) = test_state(true);

    // Request validation (format, length, casing) is the API layer's
    // job; the service applies business rules only.
    let created = state
        .user_service
        .create(command("Jane", "not-an-email"))
        .await
        .unwrap();
    assert_eq!(created.email, "not-an-email");
    let stored = repository.find_by_id(created.id).await.unwrap().unwrap();
    assert_eq!(stored.email, "not-an-email");
}

#[tokio::test]
async fn get_uses_cache_aside() {
    let (state, repository, cache) = test_state(true);
    let id = Uuid::from_u128(42);
    repository.seed(user(id, "Cached", "cached@example.com"));

    // First read: cache miss → database read → stored in cache.
    let first = state.user_service.get(id).await.unwrap();
    assert_eq!(first.email, "cached@example.com");
    assert_eq!(cache.misses.load(Ordering::SeqCst), 1);
    assert_eq!(cache.hits.load(Ordering::SeqCst), 0);
    assert_eq!(repository.db_reads.load(Ordering::SeqCst), 1);

    // Second read: cache hit → no additional database read.
    let second = state.user_service.get(id).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(cache.hits.load(Ordering::SeqCst), 1);
    assert_eq!(repository.db_reads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn get_unknown_id_is_not_found() {
    let (state, repository, _) = test_state(true);

    let error = state.user_service.get(Uuid::nil()).await.unwrap_err();
    assert!(matches!(error, AppError::NotFound(_)));
    assert_eq!(repository.db_reads.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn delete_removes_user_and_invalidates_cache() {
    let (state, repository, cache) = test_state(true);

    let created = state
        .user_service
        .create(command("Jane", "jane@example.com"))
        .await
        .unwrap();
    // Ensure the entry is cached.
    assert!(state.user_service.get(created.id).await.is_ok());
    assert!(cache.get(created.id).await.unwrap().is_some());

    state.user_service.delete(created.id).await.unwrap();

    assert!(repository.find_by_id(created.id).await.unwrap().is_none());
    assert!(cache.get(created.id).await.unwrap().is_none());
}

#[tokio::test]
async fn delete_unknown_id_is_not_found() {
    let (state, _, cache) = test_state(true);

    let error = state.user_service.delete(Uuid::nil()).await.unwrap_err();
    assert!(matches!(error, AppError::NotFound(_)));
    assert_eq!(cache.hits.load(Ordering::SeqCst), 0);
}
