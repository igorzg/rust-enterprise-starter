use utoipa::OpenApi;

use crate::api::dto::{CreateUserRequest, UserResponse};
use crate::api::error::{ErrorBody, ErrorResponse};
use crate::api::handlers::user_handler;

/// OpenAPI document for the API, served at `/openapi.json` and rendered
/// by Swagger UI at `/swagger-ui`.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "RStarter API",
        version = "0.1.0",
        description = "Production-ready Rust REST API starter with PostgreSQL, Redis cache-aside, and OpenAPI."
    ),
    paths(
        user_handler::create_user,
        user_handler::get_user,
        user_handler::delete_user
    ),
    components(
        schemas(
            CreateUserRequest,
            UserResponse,
            ErrorResponse,
            ErrorBody
        )
    ),
    tags(
        (name = "users", description = "User management")
    )
)]
pub struct ApiDoc;
