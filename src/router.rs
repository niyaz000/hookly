use axum::{
    extract::Request,
    middleware::{self, Next},
    response::Response,
    routing::get,
    Router,
};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::error::{AppError, REQUEST_ID};
use crate::features::{
    applications, endpoints, event_types, events, invites, organizations, schedules, teams,
    tenants, users,
};
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .nest("/api", api_routes(state))
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(set_request_id))
}

fn api_routes(state: AppState) -> Router {
    Router::new()
        .nest("/v1", v1_routes(state.clone()))
        .route("/health", get(health_check))
}

fn v1_routes(state: AppState) -> Router {
    Router::new()
        .merge(organizations::routes::routes(state.clone()))
        .merge(tenants::routes::routes(state.clone()))
        .merge(users::routes::routes(state.clone()))
        .merge(teams::routes::routes(state.clone()))
        .merge(invites::routes::routes(state.clone()))
        .merge(schedules::routes::routes(state.clone()))
        .merge(applications::routes::routes(state.clone()))
        .merge(event_types::routes::routes(state.clone()))
        .merge(endpoints::routes::routes(state.clone()))
        .merge(events::routes::routes(state))
}

async fn set_request_id(req: Request, next: Next) -> Response {
    let id = Uuid::now_v7();
    REQUEST_ID.scope(id, next.run(req)).await
}

async fn health_check() -> &'static str {
    "OK"
}

async fn not_found() -> AppError {
    AppError::NotFound("The requested resource was not found".to_owned())
}
