use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/roles",
            post(super::handlers::create_role)
                .layer(SetHandlerName::of(&super::handlers::create_role)),
        )
        .route(
            "/roles",
            get(super::handlers::list_roles)
                .layer(SetHandlerName::of(&super::handlers::list_roles)),
        )
        .route(
            "/roles/:id",
            get(super::handlers::get_role)
                .layer(SetHandlerName::of(&super::handlers::get_role)),
        )
        .route(
            "/roles/:id",
            patch(super::handlers::update_role)
                .layer(SetHandlerName::of(&super::handlers::update_role)),
        )
        .route(
            "/roles/:id",
            delete(super::handlers::delete_role)
                .layer(SetHandlerName::of(&super::handlers::delete_role)),
        )
        .route(
            "/roles/:id/permissions",
            get(super::handlers::list_role_permissions)
                .layer(SetHandlerName::of(&super::handlers::list_role_permissions)),
        )
        .route(
            "/roles/:id/permissions",
            post(super::handlers::assign_role_permissions)
                .layer(SetHandlerName::of(&super::handlers::assign_role_permissions)),
        )
        .route(
            "/roles/:id/permissions/:perm_id",
            delete(super::handlers::remove_role_permission)
                .layer(SetHandlerName::of(&super::handlers::remove_role_permission)),
        )
        .with_state(state)
}
