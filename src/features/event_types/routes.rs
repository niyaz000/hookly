use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/event-types",
            post(super::handlers::create_event_type).get(super::handlers::list_event_types),
        )
        .route(
            "/event-types/:public_id",
            get(super::handlers::get_event_type)
                .patch(super::handlers::update_event_type)
                .delete(super::handlers::delete_event_type),
        )
        .route(
            "/event-types/:public_id/versions",
            post(super::handlers::create_version).get(super::handlers::get_versions),
        )
        .route(
            "/event-types/:public_id/schema",
            get(super::handlers::get_schema),
        )
        .route(
            "/event-types/:public_id/archive",
            post(super::handlers::archive_event_type),
        )
        .route(
            "/event-types/:public_id/unarchive",
            post(super::handlers::unarchive_event_type),
        )
        .with_state(state)
}
