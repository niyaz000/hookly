use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/permissions",
            post(super::handlers::create_permission)
                .layer(SetHandlerName::of(&super::handlers::create_permission)),
        )
        .route(
            "/permissions",
            get(super::handlers::list_permissions)
                .layer(SetHandlerName::of(&super::handlers::list_permissions)),
        )
        .route(
            "/permissions/:id",
            get(super::handlers::get_permission)
                .layer(SetHandlerName::of(&super::handlers::get_permission)),
        )
        .route(
            "/permissions/:id",
            patch(super::handlers::update_permission)
                .layer(SetHandlerName::of(&super::handlers::update_permission)),
        )
        .route(
            "/permissions/:id",
            delete(super::handlers::delete_permission)
                .layer(SetHandlerName::of(&super::handlers::delete_permission)),
        )
        .with_state(state)
}
