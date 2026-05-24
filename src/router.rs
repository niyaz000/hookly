use axum::{Router, routing::get};
use tower_http::trace::TraceLayer;
use crate::state::AppState;
use crate::features::users;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .nest("/api", api_routes(state))
        .layer(TraceLayer::new_for_http())
}

fn api_routes(state: AppState) -> Router {
    Router::new()
        .merge(users::routes::routes(state.clone()))
        .route("/health", get(health_check))
}

async fn health_check() -> &'static str {
    "OK"
}
