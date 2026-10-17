use utoipa::OpenApi;

use crate::api::error::{ErrorBody, ErrorResponse};
use crate::api::v1::dto::{
    AssignGroupMemberRequest, CreateGroupRequest, CreateUserRequest, GroupDetailResponse,
    GroupResponse, UserResponse,
};
use crate::api::v1::handlers::{group_handler, user_handler};

/// OpenAPI document for the API, served at `/openapi.json` and rendered
/// by Swagger UI at `/swagger-ui`.
///
/// `info.version` is the **API contract** version (`version::API_CONTRACT_VERSION`),
/// not the crate version — the utoipa attributes take string literals only,
/// so `tests/api_versioning.rs` asserts the two stay in sync.
#[derive(OpenApi)]
#[openapi(
    info(
        title = "RStarter API",
        version = "1.0.0",
        description = "Production-ready Rust REST API starter with PostgreSQL, Redis cache-aside, and OpenAPI. Versioned under /api/v1; see the X-API-Version response header."
    ),
    paths(
        user_handler::create_user,
        user_handler::get_user,
        user_handler::delete_user,
        group_handler::create_group,
        group_handler::list_groups,
        group_handler::get_group,
        group_handler::delete_group,
        group_handler::add_group_member,
        group_handler::remove_group_member,
        group_handler::user_groups
    ),
    components(
        schemas(
            CreateUserRequest,
            UserResponse,
            CreateGroupRequest,
            AssignGroupMemberRequest,
            GroupResponse,
            GroupDetailResponse,
            ErrorResponse,
            ErrorBody
        )
    ),
    tags(
        (name = "users", description = "User management"),
        (name = "groups", description = "Group management and membership")
    )
)]
pub struct ApiDoc;
