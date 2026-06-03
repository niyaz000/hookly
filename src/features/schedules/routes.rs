use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/schedules", get(super::handlers::list_schedules)
            .layer(SetHandlerName::of(&super::handlers::list_schedules)))
        .route("/schedules", post(super::handlers::create_schedule)
            .layer(SetHandlerName::of(&super::handlers::create_schedule)))
        .route("/schedules/:public_id", get(super::handlers::get_schedule)
            .layer(SetHandlerName::of(&super::handlers::get_schedule)))
        .route("/schedules/:public_id", patch(super::handlers::update_schedule)
            .layer(SetHandlerName::of(&super::handlers::update_schedule)))
        .route("/schedules/:public_id", delete(super::handlers::delete_schedule)
            .layer(SetHandlerName::of(&super::handlers::delete_schedule)))
        .route("/schedules/:public_id/pause", patch(super::handlers::pause_schedule)
            .layer(SetHandlerName::of(&super::handlers::pause_schedule)))
        .route("/schedules/:public_id/resume", patch(super::handlers::resume_schedule)
            .layer(SetHandlerName::of(&super::handlers::resume_schedule)))
        .route("/schedules/:public_id/restore", patch(super::handlers::restore_schedule)
            .layer(SetHandlerName::of(&super::handlers::restore_schedule)))
        .route("/schedules/:public_id/trigger", post(super::handlers::trigger_schedule)
            .layer(SetHandlerName::of(&super::handlers::trigger_schedule)))
        .route("/schedules/:public_id/executions", get(super::handlers::list_executions)
            .layer(SetHandlerName::of(&super::handlers::list_executions)))
        .route("/schedules/:public_id/executions/:exec_public_id", get(super::handlers::get_execution)
            .layer(SetHandlerName::of(&super::handlers::get_execution)))
        .with_state(state)
}
