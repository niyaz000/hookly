use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/jwt-keys",
            post(super::handlers::create_jwt_key)
                .layer(SetHandlerName::of(&super::handlers::create_jwt_key)),
        )
        .route(
            "/jwt-keys",
            get(super::handlers::list_jwt_keys)
                .layer(SetHandlerName::of(&super::handlers::list_jwt_keys)),
        )
        .route(
            "/jwt-keys/generate",
            post(super::handlers::generate_key_pair)
                .layer(SetHandlerName::of(&super::handlers::generate_key_pair)),
        )
        .route(
            "/jwt-keys/:id",
            get(super::handlers::get_jwt_key)
                .layer(SetHandlerName::of(&super::handlers::get_jwt_key)),
        )
        .route(
            "/jwt-keys/:id",
            patch(super::handlers::update_jwt_key)
                .layer(SetHandlerName::of(&super::handlers::update_jwt_key)),
        )
        .route(
            "/jwt-keys/:id",
            delete(super::handlers::delete_jwt_key)
                .layer(SetHandlerName::of(&super::handlers::delete_jwt_key)),
        )
        .route(
            "/jwt-keys/:id/rotate",
            post(super::handlers::rotate_jwt_key)
                .layer(SetHandlerName::of(&super::handlers::rotate_jwt_key)),
        )
        .route(
            "/jwt-keys/:id/enable",
            post(super::handlers::enable_jwt_key)
                .layer(SetHandlerName::of(&super::handlers::enable_jwt_key)),
        )
        .route(
            "/jwt-keys/:id/disable",
            post(super::handlers::disable_jwt_key)
                .layer(SetHandlerName::of(&super::handlers::disable_jwt_key)),
        )
        .route(
            "/jwt-keys/:id/public-key",
            get(super::handlers::get_public_key)
                .layer(SetHandlerName::of(&super::handlers::get_public_key)),
        )
        .with_state(state.clone())
        // JWKS endpoint is merged separately (no state needed but keep pattern consistent)
        .merge(jwks_route(state))
}

fn jwks_route(state: AppState) -> Router {
    Router::new()
        .route(
            "/.well-known/jwks.json",
            get(super::handlers::get_jwks)
                .layer(SetHandlerName::of(&super::handlers::get_jwks)),
        )
        .with_state(state)
}
