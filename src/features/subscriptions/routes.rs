use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/subscriptions",
            post(super::handlers::create_subscription)
                .layer(SetHandlerName::of(&super::handlers::create_subscription)),
        )
        .route(
            "/subscriptions",
            get(super::handlers::list_subscriptions)
                .layer(SetHandlerName::of(&super::handlers::list_subscriptions)),
        )
        .route(
            "/subscriptions/:sub_id",
            get(super::handlers::get_subscription)
                .layer(SetHandlerName::of(&super::handlers::get_subscription)),
        )
        .route(
            "/subscriptions/:sub_id",
            delete(super::handlers::delete_subscription)
                .layer(SetHandlerName::of(&super::handlers::delete_subscription)),
        )
        .with_state(state)
}
