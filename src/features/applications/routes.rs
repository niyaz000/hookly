use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/applications", post(super::handlers::create_application)
            .layer(SetHandlerName::of(&super::handlers::create_application)))
        .route("/applications/:public_id", get(super::handlers::get_by_id)
            .layer(SetHandlerName::of(&super::handlers::get_by_id)))
        .route("/applications/:public_id", delete(super::handlers::delete_by_id)
            .layer(SetHandlerName::of(&super::handlers::delete_by_id)))
        .route("/applications/:public_id/restore", post(super::handlers::restore_by_id)
            .layer(SetHandlerName::of(&super::handlers::restore_by_id)))
        .with_state(state)
}
