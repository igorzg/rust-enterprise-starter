use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::api::dto::{
    AssignGroupMemberRequest, CreateGroupRequest, GroupDetailResponse, GroupResponse,
};
use crate::api::error::{ApiError, ErrorResponse};
use crate::state::AppState;

/// Create a group.
#[utoipa::path(
    post,
    path = "/api/v1/groups",
    request_body = CreateGroupRequest,
    responses(
        (status = 201, description = "Group created", body = GroupResponse),
        (status = 400, description = "Invalid request body", body = ErrorResponse),
        (status = 409, description = "A group with this name already exists", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "groups"
)]
pub async fn create_group(
    State(state): State<AppState>,
    result: Result<Json<CreateGroupRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<GroupResponse>), ApiError> {
    let request = result.map_err(|rejection| {
        tracing::warn!(%rejection, "invalid JSON body rejected");
        ApiError::InvalidBody
    })?;
    let command = request.to_command()?;
    let group = state.group_service.create(command).await?;
    Ok((StatusCode::CREATED, Json(GroupResponse::from(&group))))
}

/// List all groups.
#[utoipa::path(
    get,
    path = "/api/v1/groups",
    responses(
        (status = 200, description = "Groups listed", body = Vec<GroupResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "groups"
)]
pub async fn list_groups(
    State(state): State<AppState>,
) -> Result<Json<Vec<GroupResponse>>, ApiError> {
    let groups = state.group_service.list().await?;
    Ok(Json(groups.iter().map(GroupResponse::from).collect()))
}

/// Fetch a group by id together with its current members.
#[utoipa::path(
    get,
    path = "/api/v1/groups/{id}",
    params(
        ("id" = Uuid, description = "Group identifier")
    ),
    responses(
        (status = 200, description = "Group found", body = GroupDetailResponse),
        (status = 400, description = "Invalid id", body = ErrorResponse),
        (status = 404, description = "Group not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "groups"
)]
pub async fn get_group(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<GroupDetailResponse>, ApiError> {
    let id = id.map_err(|_| ApiError::InvalidId)?.0;
    let (group, members) = state.group_service.get_with_members(id).await?;
    Ok(Json(GroupDetailResponse::from_parts(&group, &members)))
}

/// Delete a group (and its membership rows, via the database).
#[utoipa::path(
    delete,
    path = "/api/v1/groups/{id}",
    params(
        ("id" = Uuid, description = "Group identifier")
    ),
    responses(
        (status = 204, description = "Group deleted"),
        (status = 400, description = "Invalid id", body = ErrorResponse),
        (status = 404, description = "Group not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "groups"
)]
pub async fn delete_group(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    let id = id.map_err(|_| ApiError::InvalidId)?.0;
    state.group_service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Assign a user to a group.
#[utoipa::path(
    post,
    path = "/api/v1/groups/{id}/users",
    params(
        ("id" = Uuid, description = "Group identifier")
    ),
    request_body = AssignGroupMemberRequest,
    responses(
        (status = 204, description = "User assigned to group"),
        (status = 400, description = "Invalid id or request body", body = ErrorResponse),
        (status = 404, description = "Group or user not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "groups"
)]
pub async fn add_group_member(
    State(state): State<AppState>,
    group_id: Result<Path<Uuid>, PathRejection>,
    result: Result<Json<AssignGroupMemberRequest>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let group_id = group_id.map_err(|_| ApiError::InvalidId)?.0;
    let request = result.map_err(|_| ApiError::InvalidBody)?;
    state
        .group_service
        .add_member(group_id, request.user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Remove a user from a group.
#[utoipa::path(
    delete,
    path = "/api/v1/groups/{id}/users/{userId}",
    params(
        ("id" = Uuid, description = "Group identifier"),
        ("userId" = Uuid, description = "User identifier")
    ),
    responses(
        (status = 204, description = "User removed from group"),
        (status = 400, description = "Invalid id", body = ErrorResponse),
        (status = 404, description = "Group, user, or membership not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "groups"
)]
pub async fn remove_group_member(
    State(state): State<AppState>,
    ids: Result<Path<(Uuid, Uuid)>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    let (group_id, user_id) = ids.map_err(|_| ApiError::InvalidId)?.0;
    state.group_service.remove_member(group_id, user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// List the groups a user belongs to.
#[utoipa::path(
    get,
    path = "/api/v1/users/{id}/groups",
    params(
        ("id" = Uuid, description = "User identifier")
    ),
    responses(
        (status = 200, description = "Groups the user belongs to", body = Vec<GroupResponse>),
        (status = 400, description = "Invalid id", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "groups"
)]
pub async fn user_groups(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<Vec<GroupResponse>>, ApiError> {
    let id = id.map_err(|_| ApiError::InvalidId)?.0;
    let groups = state.group_service.groups_for_user(id).await?;
    Ok(Json(groups.iter().map(GroupResponse::from).collect()))
}
