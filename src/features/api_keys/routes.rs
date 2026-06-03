use axum::{
    routing::{delete, get, patch, post, put},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        // API key CRUD
        .route("/api-keys", post(super::handlers::create_api_key)
            .layer(SetHandlerName::of(&super::handlers::create_api_key)))
        .route("/api-keys", get(super::handlers::list_api_keys)
            .layer(SetHandlerName::of(&super::handlers::list_api_keys)))
        .route("/api-keys/:id", get(super::handlers::get_api_key)
            .layer(SetHandlerName::of(&super::handlers::get_api_key)))
        .route("/api-keys/:id", patch(super::handlers::update_api_key)
            .layer(SetHandlerName::of(&super::handlers::update_api_key)))
        .route("/api-keys/:id", delete(super::handlers::delete_api_key)
            .layer(SetHandlerName::of(&super::handlers::delete_api_key)))
        .route("/api-keys/:id/reveal", get(super::handlers::reveal_api_key)
            .layer(SetHandlerName::of(&super::handlers::reveal_api_key)))
        // Settings
        .route("/api-key-settings", post(super::handlers::upsert_api_key_settings)
            .layer(SetHandlerName::of(&super::handlers::upsert_api_key_settings)))
        .route("/api-key-settings/:id", get(super::handlers::get_api_key_settings)
            .layer(SetHandlerName::of(&super::handlers::get_api_key_settings)))
        .route("/api-key-settings/:id", put(super::handlers::update_api_key_settings)
            .layer(SetHandlerName::of(&super::handlers::update_api_key_settings)))
        .with_state(state)
}
