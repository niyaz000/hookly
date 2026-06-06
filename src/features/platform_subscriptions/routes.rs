use axum::{
    routing::{delete, get, post, put},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/platform-subscriptions",
            get(super::handlers::list_subscriptions)
                .layer(SetHandlerName::of(&super::handlers::list_subscriptions)),
        )
        .route(
            "/platform-subscriptions",
            post(super::handlers::subscribe)
                .layer(SetHandlerName::of(&super::handlers::subscribe)),
        )
        .route(
            "/platform-subscriptions",
            put(super::handlers::replace_subscriptions)
                .layer(SetHandlerName::of(&super::handlers::replace_subscriptions)),
        )
        .route(
            "/platform-subscriptions",
            delete(super::handlers::unsubscribe)
                .layer(SetHandlerName::of(&super::handlers::unsubscribe)),
        )
        .with_state(state)
}
