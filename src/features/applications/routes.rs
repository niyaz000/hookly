use axum::{routing::post, Router};

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/applications", post(super::handlers::create_application))
        .with_state(state)
}
