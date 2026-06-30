use axum::{
    extract::Request,
    http::{header::CONTENT_LENGTH, HeaderName, HeaderValue},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use tower_http::trace::TraceLayer;
use tracing::Instrument;
use uuid::Uuid;

use crate::common::{access_log, call_counter, types::RequestContext};
use crate::error::{AppError, REQUEST_ID, REQUEST_PATH};
use crate::features::{
    api_keys, applications, assignments, delivery, endpoints, environments, event_types, events,
    invites, jwt_keys, organizations, permissions, platform, platform_event_types,
    platform_subscriptions, platform_webhooks, roles, schedules, subscriptions, teams, tenants,
    users,
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
    // Token-based flows that pre-date or bypass API key auth (invite recipients).
    let public = invites::routes::public_routes(state.clone());

    // Admin routes — authenticated by the ADMIN_API_KEY env var.
    // Capabilities: create org, restore suspended org, suspend/reactivate any tenant.
    let admin_state = state.clone();
    let admin = Router::new()
        .nest("/admin", platform::routes::routes(state.clone()))
        .route_layer(middleware::from_fn(move |req: Request, next: Next| {
            let s = admin_state.clone();
            async move { platform::auth::authenticate_admin(s, req, next).await }
        }));

    // Every protected route goes through the authenticate middleware which:
    //   1. Validates the Bearer API key or JWT (Redis cache → DB JOIN on miss)
    //   2. Checks org/tenant status (suspended/deleted → 403)
    //   3. Injects ApiKeyPrincipal + RequestContext into extensions
    let auth_state = state.clone();
    let protected = Router::new()
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
        .merge(delivery::routes::routes(state.clone()))
        .merge(api_keys::routes::routes(state.clone()))
        .merge(environments::routes::routes(state.clone()))
        .merge(permissions::routes::routes(state.clone()))
        .merge(roles::routes::routes(state.clone()))
        .merge(assignments::routes::routes(state.clone()))
        .merge(jwt_keys::routes::routes(state.clone()))
        .merge(platform_event_types::routes::routes(state.clone()))
        .merge(platform_webhooks::routes::routes(state.clone()))
        .merge(platform_subscriptions::routes::routes(state.clone()))
        .merge(subscriptions::routes::routes(state.clone()))
        .route_layer(middleware::from_fn(move |req: Request, next: Next| {
            let s = auth_state.clone();
            async move { api_keys::auth::authenticate(s, req, next).await }
        }));

    Router::new()
        .merge(public)
        .merge(admin)
        .merge(protected)
        .route_layer(middleware::from_fn(inject_request_context))
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
    let path = req.uri().path().to_owned();
    let span = tracing::info_span!("http_request", request_id = %id);
    let mut response = REQUEST_ID
        .scope(
            id,
            REQUEST_PATH.scope(path, call_counter::scoped(next.run(req))),
        )
        .instrument(span)
        .await;
    // Safety: UUID is always valid ASCII
    let val = HeaderValue::from_str(&id.to_string()).expect("uuid is valid header value");
    response
        .headers_mut()
        .insert(HeaderName::from_static("x-request-id"), val);
    response
}

// Sets a default RequestContext for public routes. For protected routes the
// authenticate middleware overwrites this with the real principal values.
async fn inject_request_context(mut req: Request, next: Next) -> Response {
    let request_id = REQUEST_ID
        .try_with(|id| *id)
        .unwrap_or_else(|_| Uuid::now_v7());
    req.extensions_mut().insert(RequestContext {
        request_id,
        created_by: Uuid::nil(),
        organization_id: Uuid::nil(),
        tenant_id: Uuid::nil(),
    });
    next.run(req).await
}

async fn health_check() -> &'static str {
    "OK"
}

async fn not_found() -> AppError {
    AppError::NotFound("The requested resource was not found".to_owned())
}
