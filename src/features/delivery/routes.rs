use axum::{routing::post, Router};

use crate::{features::delivery::handlers, state::AppState};

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/delivery-jobs/{public_id}/retry",
            post(handlers::retry_delivery_job),
        )
        .with_state(state)
}
