use axum::{
    routing::{get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/environments", post(super::handlers::create_environment)
            .layer(SetHandlerName::of(&super::handlers::create_environment)))
        .route("/environments", get(super::handlers::list_environments)
            .layer(SetHandlerName::of(&super::handlers::list_environments)))
        .route("/environments/:id", get(super::handlers::get_environment)
            .layer(SetHandlerName::of(&super::handlers::get_environment)))
        .route("/environments/:id", patch(super::handlers::update_environment)
            .layer(SetHandlerName::of(&super::handlers::update_environment)))
        .route("/environments/:id/enable", post(super::handlers::enable_environment)
            .layer(SetHandlerName::of(&super::handlers::enable_environment)))
        .route("/environments/:id/disable", post(super::handlers::disable_environment)
            .layer(SetHandlerName::of(&super::handlers::disable_environment)))
        .with_state(state)
}
