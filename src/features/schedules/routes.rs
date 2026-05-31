use axum::{
    routing::{get, patch, post},
    Router,
};

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/schedules",
            get(super::handlers::list_schedules).post(super::handlers::create_schedule),
        )
        .route(
            "/schedules/:public_id",
            get(super::handlers::get_schedule)
                .patch(super::handlers::update_schedule)
                .delete(super::handlers::delete_schedule),
        )
        .route(
            "/schedules/:public_id/pause",
            patch(super::handlers::pause_schedule),
        )
        .route(
            "/schedules/:public_id/resume",
            patch(super::handlers::resume_schedule),
        )
        .route(
            "/schedules/:public_id/restore",
            patch(super::handlers::restore_schedule),
        )
        .route(
            "/schedules/:public_id/trigger",
            post(super::handlers::trigger_schedule),
        )
        .route(
            "/schedules/:public_id/executions",
            get(super::handlers::list_executions),
        )
        .route(
            "/schedules/:public_id/executions/:exec_public_id",
            get(super::handlers::get_execution),
        )
        .with_state(state)
}
