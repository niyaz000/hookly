use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/users",
            get(super::handlers::list_users).post(super::handlers::create_user),
        )
        .route(
            "/users/:public_id",
            get(super::handlers::get_user)
                .patch(super::handlers::update_user)
                .delete(super::handlers::delete_user),
        )
        .route(
            "/users/:public_id/suspend",
            post(super::handlers::suspend_user),
        )
        .route(
            "/users/:public_id/reactivate",
            post(super::handlers::reactivate_user),
        )
        .route("/users/:public_id/lock", post(super::handlers::lock_user))
        .route(
            "/users/:public_id/unlock",
            post(super::handlers::unlock_user),
        )
        .with_state(state)
}
