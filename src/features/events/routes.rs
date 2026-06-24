use axum::{
    routing::{get, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/events", post(super::handlers::create_event)
            .layer(SetHandlerName::of(&super::handlers::create_event)))
        .route("/events", get(super::handlers::list_events)
            .layer(SetHandlerName::of(&super::handlers::list_events)))
        .route("/events/bulk", post(super::handlers::create_events_bulk)
            .layer(SetHandlerName::of(&super::handlers::create_events_bulk)))
        .route("/events/:evt_id", get(super::handlers::get_event)
            .layer(SetHandlerName::of(&super::handlers::get_event)))
        .with_state(state)
}
