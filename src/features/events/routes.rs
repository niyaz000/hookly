use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/events",
            post(super::handlers::create_event).get(super::handlers::list_events),
        )
        .route("/events/:evt_id", get(super::handlers::get_event))
        .with_state(state)
}
