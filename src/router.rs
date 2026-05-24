use axum::{routing::get, Router};
use tower_http::trace::TraceLayer;

use crate::features::{applications, users};
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .nest("/api", api_routes(state))
        .layer(TraceLayer::new_for_http())
}

fn api_routes(state: AppState) -> Router {
    Router::new()
        .merge(users::routes::routes(state.clone()))
        .nest("/v1", v1_routes(state.clone()))
        .route("/health", get(health_check))
}

fn v1_routes(state: AppState) -> Router {
    Router::new().merge(applications::routes::routes(state))
}

async fn health_check() -> &'static str {
    "OK"
}
