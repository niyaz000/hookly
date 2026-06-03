use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/organizations", get(super::handlers::list_organizations)
            .layer(SetHandlerName::of(&super::handlers::list_organizations)))
        .route("/organizations", post(super::handlers::create_organization)
            .layer(SetHandlerName::of(&super::handlers::create_organization)))
        .route("/organizations/:public_id", get(super::handlers::get_organization)
            .layer(SetHandlerName::of(&super::handlers::get_organization)))
        .route("/organizations/:public_id", patch(super::handlers::update_organization)
            .layer(SetHandlerName::of(&super::handlers::update_organization)))
        .route("/organizations/:public_id", delete(super::handlers::delete_organization)
            .layer(SetHandlerName::of(&super::handlers::delete_organization)))
        .route("/organizations/:public_id/suspend", post(super::handlers::suspend_organization)
            .layer(SetHandlerName::of(&super::handlers::suspend_organization)))
        .route("/organizations/:public_id/restore", post(super::handlers::restore_organization)
            .layer(SetHandlerName::of(&super::handlers::restore_organization)))
        .with_state(state)
}
