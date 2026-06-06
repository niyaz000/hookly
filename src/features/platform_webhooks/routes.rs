use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/platform-webhooks",
            post(super::handlers::create_platform_webhook)
                .layer(SetHandlerName::of(&super::handlers::create_platform_webhook)),
        )
        .route(
            "/platform-webhooks",
            get(super::handlers::list_platform_webhooks)
                .layer(SetHandlerName::of(&super::handlers::list_platform_webhooks)),
        )
        .route(
            "/platform-webhooks/:id",
            get(super::handlers::get_platform_webhook)
                .layer(SetHandlerName::of(&super::handlers::get_platform_webhook)),
        )
        .route(
            "/platform-webhooks/:id",
            patch(super::handlers::update_platform_webhook)
                .layer(SetHandlerName::of(&super::handlers::update_platform_webhook)),
        )
        .route(
            "/platform-webhooks/:id",
            delete(super::handlers::delete_platform_webhook)
                .layer(SetHandlerName::of(&super::handlers::delete_platform_webhook)),
        )
        .route(
            "/platform-webhooks/:id/suspend",
            post(super::handlers::suspend_platform_webhook)
                .layer(SetHandlerName::of(&super::handlers::suspend_platform_webhook)),
        )
        .route(
            "/platform-webhooks/:id/activate",
            post(super::handlers::activate_platform_webhook)
                .layer(SetHandlerName::of(&super::handlers::activate_platform_webhook)),
        )
        .route(
            "/platform-webhooks/:id/rotate-secret",
            post(super::handlers::rotate_platform_webhook_secret)
                .layer(SetHandlerName::of(&super::handlers::rotate_platform_webhook_secret)),
        )
        .with_state(state)
}
