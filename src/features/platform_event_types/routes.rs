use axum::{routing::get, Router};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/platform-event-types",
            get(super::handlers::list_platform_event_types)
                .layer(SetHandlerName::of(&super::handlers::list_platform_event_types)),
        )
        .route(
            "/platform-event-types/:id",
            get(super::handlers::get_platform_event_type)
                .layer(SetHandlerName::of(&super::handlers::get_platform_event_type)),
        )
        .with_state(state)
}
