//! Unit tests for the GroupService (business rules, cache-aside,
//! invalidation, membership). Request validation is covered by the API
//! tests.

use std::sync::atomic::Ordering;

use uuid::Uuid;

use rstarter::services::domain::{CreateGroupCommand, Group};
use rstarter::services::errors::{AppError, NotFoundError};
use rstarter::services::ports::{GroupCache, GroupRepository, UserRepository};

mod common;

use common::test_state_full;

fn group(id: Uuid, name: &str) -> Group {
    Group {
        id,
        name: name.to_string(),
    }
}

fn command(name: &str) -> CreateGroupCommand {
    CreateGroupCommand {
        name: name.to_string(),
    }
}

#[tokio::test]
async fn create_persists_group_and_populates_cache() {
    let (state, _, _, group_repository, group_cache) = test_state_full(true);

    let created = state.group_service.create(command("admins")).await.unwrap();

    assert!(!created.id.is_nil());
    assert_eq!(created.name, "admins");
    let stored = group_repository
        .find_by_id(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored, created);
    let cached = group_cache.get(created.id).await.unwrap().unwrap();
    assert_eq!(cached, created);
}

#[tokio::test]
async fn create_rejects_duplicate_name() {
    let (state, _, _, _, _) = test_state_full(true);

    state.group_service.create(command("admins")).await.unwrap();

    let error = state
        .group_service
        .create(command("admins"))
        .await
        .unwrap_err();
    assert!(matches!(error, AppError::Conflict(_)));
}

#[tokio::test]
async fn list_returns_all_groups() {
    let (state, _, _, group_repository, _) = test_state_full(true);

    group_repository.seed(group(Uuid::from_u128(1), "alpha"));
    group_repository.seed(group(Uuid::from_u128(2), "beta"));

    let groups = state.group_service.list().await.unwrap();
    let mut names: Vec<String> = groups.iter().map(|g| g.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["alpha", "beta"]);
}

#[tokio::test]
async fn get_uses_cache_aside() {
    let (state, _, _, group_repository, group_cache) = test_state_full(true);
    let id = Uuid::from_u128(42);
    group_repository.seed(group(id, "cached"));

    // First read: cache miss → database read → stored in cache.
    let first = state.group_service.get(id).await.unwrap();
    assert_eq!(first.name, "cached");
    assert_eq!(group_cache.misses.load(Ordering::SeqCst), 1);
    assert_eq!(group_cache.hits.load(Ordering::SeqCst), 0);

    // Second read: cache hit → no additional database read.
    let second = state.group_service.get(id).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(group_cache.hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn get_unknown_id_is_not_found() {
    let (state, _, _, _, _) = test_state_full(true);

    let error = state.group_service.get(Uuid::nil()).await.unwrap_err();
    assert!(matches!(error, AppError::NotFound(_)));
}

#[tokio::test]
async fn delete_removes_group_and_invalidates_cache() {
    let (state, _, _, group_repository, group_cache) = test_state_full(true);

    let created = state.group_service.create(command("admins")).await.unwrap();
    // Ensure the entry is cached.
    assert!(state.group_service.get(created.id).await.is_ok());
    assert!(group_cache.get(created.id).await.unwrap().is_some());

    state.group_service.delete(created.id).await.unwrap();

    assert!(
        group_repository
            .find_by_id(created.id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(group_cache.get(created.id).await.unwrap().is_none());
}

#[tokio::test]
async fn delete_unknown_id_is_not_found() {
    let (state, _, _, _, _) = test_state_full(true);

    let error = state.group_service.delete(Uuid::nil()).await.unwrap_err();
    assert!(matches!(error, AppError::NotFound(_)));
}

#[tokio::test]
async fn add_member_and_list_members() {
    let (state, user_repository, _, _, _) = test_state_full(true);
    let user = user_repository
        .insert("Jane", "jane@example.com")
        .await
        .unwrap();
    let group = state.group_service.create(command("admins")).await.unwrap();

    state
        .group_service
        .add_member(group.id, user.id)
        .await
        .unwrap();
    // Idempotent: assigning the same member again is a no-op success.
    state
        .group_service
        .add_member(group.id, user.id)
        .await
        .unwrap();

    let (fetched, members) = state
        .group_service
        .get_with_members(group.id)
        .await
        .unwrap();
    assert_eq!(fetched.id, group.id);
    assert_eq!(members, vec![user.clone()]);

    let groups = state.group_service.groups_for_user(user.id).await.unwrap();
    assert_eq!(groups, vec![group.clone()]);
}

#[tokio::test]
async fn add_member_requires_existing_group_and_user() {
    let (state, user_repository, _, _, _) = test_state_full(true);
    let user = user_repository
        .insert("Jane", "jane@example.com")
        .await
        .unwrap();

    let error = state
        .group_service
        .add_member(Uuid::nil(), user.id)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        AppError::NotFound(NotFoundError::Group { .. })
    ));

    let group = state.group_service.create(command("admins")).await.unwrap();
    let error = state
        .group_service
        .add_member(group.id, Uuid::nil())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        AppError::NotFound(NotFoundError::User { .. })
    ));
}

#[tokio::test]
async fn remove_member_and_groups_for_user() {
    let (state, user_repository, _, _, _) = test_state_full(true);
    let alice = user_repository
        .insert("Alice", "alice@example.com")
        .await
        .unwrap();
    let bob = user_repository
        .insert("Bob", "bob@example.com")
        .await
        .unwrap();
    let group = state.group_service.create(command("admins")).await.unwrap();

    state
        .group_service
        .add_member(group.id, alice.id)
        .await
        .unwrap();
    state
        .group_service
        .add_member(group.id, bob.id)
        .await
        .unwrap();

    state
        .group_service
        .remove_member(group.id, bob.id)
        .await
        .unwrap();

    let groups = state.group_service.groups_for_user(bob.id).await.unwrap();
    assert!(groups.is_empty());
    let groups = state.group_service.groups_for_user(alice.id).await.unwrap();
    assert_eq!(groups, vec![group.clone()]);

    let (_, members) = state
        .group_service
        .get_with_members(group.id)
        .await
        .unwrap();
    assert_eq!(members, vec![alice.clone()]);
}

#[tokio::test]
async fn remove_member_unknown_membership_is_not_found() {
    let (state, user_repository, _, _, _) = test_state_full(true);
    let user = user_repository
        .insert("Jane", "jane@example.com")
        .await
        .unwrap();
    let group = state.group_service.create(command("admins")).await.unwrap();

    // The group and user exist, but there is no membership row.
    let error = state
        .group_service
        .remove_member(group.id, user.id)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        AppError::NotFound(NotFoundError::GroupMembership {
            group_id,
            user_id
        }) if group_id == group.id && user_id == user.id
    ));
}

#[tokio::test]
async fn groups_for_user_requires_existing_user() {
    let (state, _, _, _, _) = test_state_full(true);

    let error = state
        .group_service
        .groups_for_user(Uuid::nil())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        AppError::NotFound(NotFoundError::User { .. })
    ));
}
