use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/endpoints", post(super::handlers::create_endpoint)
            .layer(SetHandlerName::of(&super::handlers::create_endpoint)))
        .route("/endpoints", get(super::handlers::list_endpoints)
            .layer(SetHandlerName::of(&super::handlers::list_endpoints)))
        .route("/endpoints/:ep_id", get(super::handlers::get_endpoint)
            .layer(SetHandlerName::of(&super::handlers::get_endpoint)))
        .route("/endpoints/:ep_id", patch(super::handlers::update_endpoint)
            .layer(SetHandlerName::of(&super::handlers::update_endpoint)))
        .route("/endpoints/:ep_id", delete(super::handlers::delete_endpoint)
            .layer(SetHandlerName::of(&super::handlers::delete_endpoint)))
        .route("/endpoints/:ep_id/pause", post(super::handlers::pause_endpoint)
            .layer(SetHandlerName::of(&super::handlers::pause_endpoint)))
        .route("/endpoints/:ep_id/resume", post(super::handlers::resume_endpoint)
            .layer(SetHandlerName::of(&super::handlers::resume_endpoint)))
        .route("/endpoints/:ep_id/secret", get(super::handlers::get_secret)
            .layer(SetHandlerName::of(&super::handlers::get_secret)))
        .route("/endpoints/:ep_id/secret/rotate", post(super::handlers::rotate_secret)
            .layer(SetHandlerName::of(&super::handlers::rotate_secret)))
        .with_state(state)
}
