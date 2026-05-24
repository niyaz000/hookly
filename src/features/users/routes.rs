use axum::{Router, routing::post};
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/users", post(super::handlers::create_user))
        .with_state(state)
}
