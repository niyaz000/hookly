use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/users", get(super::handlers::list_users)
            .layer(SetHandlerName::of(&super::handlers::list_users)))
        .route("/users", post(super::handlers::create_user)
            .layer(SetHandlerName::of(&super::handlers::create_user)))
        .route("/users/:public_id", get(super::handlers::get_user)
            .layer(SetHandlerName::of(&super::handlers::get_user)))
        .route("/users/:public_id", patch(super::handlers::update_user)
            .layer(SetHandlerName::of(&super::handlers::update_user)))
        .route("/users/:public_id", delete(super::handlers::delete_user)
            .layer(SetHandlerName::of(&super::handlers::delete_user)))
        .route("/users/:public_id/suspend", post(super::handlers::suspend_user)
            .layer(SetHandlerName::of(&super::handlers::suspend_user)))
        .route("/users/:public_id/reactivate", post(super::handlers::reactivate_user)
            .layer(SetHandlerName::of(&super::handlers::reactivate_user)))
        .route("/users/:public_id/lock", post(super::handlers::lock_user)
            .layer(SetHandlerName::of(&super::handlers::lock_user)))
        .route("/users/:public_id/unlock", post(super::handlers::unlock_user)
            .layer(SetHandlerName::of(&super::handlers::unlock_user)))
        .with_state(state)
}
