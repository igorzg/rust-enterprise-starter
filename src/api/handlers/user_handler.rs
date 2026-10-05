use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::api::dto::{CreateUserRequest, UserResponse};
use crate::api::error::{ApiError, ErrorResponse};
use crate::state::AppState;

/// Create a user.
#[utoipa::path(
    post,
    path = "/api/v1/users",
    request_body = CreateUserRequest,
    responses(
        (status = 201, description = "User created", body = UserResponse),
        (status = 400, description = "Invalid request body", body = ErrorResponse),
        (status = 409, description = "A user with this email already exists", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "users"
)]
pub async fn create_user(
    State(state): State<AppState>,
    result: Result<Json<CreateUserRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<UserResponse>), ApiError> {
    let request = result.map_err(|rejection| {
        tracing::warn!(%rejection, "invalid JSON body rejected");
        ApiError::InvalidBody
    })?;
    let command = request.to_command()?;
    let user = state.user_service.create(command).await?;
    Ok((StatusCode::CREATED, Json(UserResponse::from(&user))))
}

/// Fetch a user by id (cache-aside: Redis hit or PostgreSQL fallback).
#[utoipa::path(
    get,
    path = "/api/v1/users/{id}",
    params(
        ("id" = Uuid, description = "User identifier")
    ),
    responses(
        (status = 200, description = "User found", body = UserResponse),
        (status = 400, description = "Invalid id", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "users"
)]
pub async fn get_user(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<Json<UserResponse>, ApiError> {
    let id = id.map_err(|_| ApiError::InvalidId)?.0;
    let user = state.user_service.get(id).await?;
    Ok(Json(UserResponse::from(&user)))
}

/// Delete a user (and invalidate the cache entry).
#[utoipa::path(
    delete,
    path = "/api/v1/users/{id}",
    params(
        ("id" = Uuid, description = "User identifier")
    ),
    responses(
        (status = 204, description = "User deleted"),
        (status = 400, description = "Invalid id", body = ErrorResponse),
        (status = 404, description = "User not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "users"
)]
pub async fn delete_user(
    State(state): State<AppState>,
    id: Result<Path<Uuid>, PathRejection>,
) -> Result<StatusCode, ApiError> {
    let id = id.map_err(|_| ApiError::InvalidId)?.0;
    state.user_service.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
