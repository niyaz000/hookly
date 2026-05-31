use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/organizations",
            get(super::handlers::list_organizations).post(super::handlers::create_organization),
        )
        .route(
            "/organizations/:public_id",
            get(super::handlers::get_organization)
                .patch(super::handlers::update_organization)
                .delete(super::handlers::delete_organization),
        )
        .route(
            "/organizations/:public_id/suspend",
            post(super::handlers::suspend_organization),
        )
        .route(
            "/organizations/:public_id/restore",
            post(super::handlers::restore_organization),
        )
        .with_state(state)
}
