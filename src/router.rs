use axum::{
    extract::Request,
    http::header::CONTENT_LENGTH,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::common::access_log;
use crate::error::{AppError, REQUEST_ID};
use crate::features::{
    api_keys, applications, endpoints, event_types, events, invites, organizations, schedules,
    teams, tenants, users,
};
use crate::state::AppState;

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .nest("/api", api_routes(state))
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(check_body_size))
        .layer(middleware::from_fn(check_uri_length))
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
        .merge(events::routes::routes(state.clone()))
        .merge(api_keys::routes::routes(state))
        .route_layer(middleware::from_fn(access_log::access_log))
}

const MAX_URI_LEN: usize = 512;
const MAX_BODY_BYTES: usize = 256 * 1024;

async fn check_uri_length(req: Request, next: Next) -> Response {
    let uri_len = req.uri().path_and_query().map_or(0, |pq| pq.as_str().len());
    if uri_len > MAX_URI_LEN {
        return AppError::UriTooLong.into_response();
    }
    next.run(req).await
}

async fn check_body_size(req: Request, next: Next) -> Response {
    if let Some(cl) = req.headers().get(CONTENT_LENGTH) {
        if let Some(len) = cl.to_str().ok().and_then(|s| s.parse::<usize>().ok()) {
            if len > MAX_BODY_BYTES {
                return AppError::PayloadTooLarge.into_response();
            }
        }
    }
    next.run(req).await
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
