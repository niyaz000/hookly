use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/endpoints",
            post(super::handlers::create_endpoint).get(super::handlers::list_endpoints),
        )
        .route(
            "/endpoints/:ep_id",
            get(super::handlers::get_endpoint)
                .patch(super::handlers::update_endpoint)
                .delete(super::handlers::delete_endpoint),
        )
        .route(
            "/endpoints/:ep_id/pause",
            post(super::handlers::pause_endpoint),
        )
        .route(
            "/endpoints/:ep_id/resume",
            post(super::handlers::resume_endpoint),
        )
        .route("/endpoints/:ep_id/secret", get(super::handlers::get_secret))
        .route(
            "/endpoints/:ep_id/secret/rotate",
            post(super::handlers::rotate_secret),
        )
        .with_state(state)
}
