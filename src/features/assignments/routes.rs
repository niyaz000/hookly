use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        // User roles
        .route(
            "/users/:id/roles",
            get(super::handlers::list_user_roles)
                .layer(SetHandlerName::of(&super::handlers::list_user_roles)),
        )
        .route(
            "/users/:id/roles",
            post(super::handlers::assign_user_roles)
                .layer(SetHandlerName::of(&super::handlers::assign_user_roles)),
        )
        .route(
            "/users/:id/roles/:role_id",
            delete(super::handlers::remove_user_role)
                .layer(SetHandlerName::of(&super::handlers::remove_user_role)),
        )
        // User permissions
        .route(
            "/users/:id/permissions",
            get(super::handlers::list_user_permissions)
                .layer(SetHandlerName::of(&super::handlers::list_user_permissions)),
        )
        .route(
            "/users/:id/permissions",
            post(super::handlers::assign_user_permissions)
                .layer(SetHandlerName::of(&super::handlers::assign_user_permissions)),
        )
        .route(
            "/users/:id/permissions/:perm_id",
            delete(super::handlers::remove_user_permission)
                .layer(SetHandlerName::of(&super::handlers::remove_user_permission)),
        )
        // User effective permissions
        .route(
            "/users/:id/effective-permissions",
            get(super::handlers::get_user_effective_permissions)
                .layer(SetHandlerName::of(&super::handlers::get_user_effective_permissions)),
        )
        // API key roles
        .route(
            "/api-keys/:id/roles",
            get(super::handlers::list_api_key_roles)
                .layer(SetHandlerName::of(&super::handlers::list_api_key_roles)),
        )
        .route(
            "/api-keys/:id/roles",
            post(super::handlers::assign_api_key_roles)
                .layer(SetHandlerName::of(&super::handlers::assign_api_key_roles)),
        )
        .route(
            "/api-keys/:id/roles/:role_id",
            delete(super::handlers::remove_api_key_role)
                .layer(SetHandlerName::of(&super::handlers::remove_api_key_role)),
        )
        // API key permissions
        .route(
            "/api-keys/:id/permissions",
            get(super::handlers::list_api_key_permissions)
                .layer(SetHandlerName::of(&super::handlers::list_api_key_permissions)),
        )
        .route(
            "/api-keys/:id/permissions",
            post(super::handlers::assign_api_key_permissions)
                .layer(SetHandlerName::of(&super::handlers::assign_api_key_permissions)),
        )
        .route(
            "/api-keys/:id/permissions/:perm_id",
            delete(super::handlers::remove_api_key_permission)
                .layer(SetHandlerName::of(&super::handlers::remove_api_key_permission)),
        )
        .with_state(state)
}
