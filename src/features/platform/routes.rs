use axum::{
    routing::{patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/organizations",
            post(super::handlers::create_organization)
                .layer(SetHandlerName::of(&super::handlers::create_organization)),
        )
        .route(
            "/organizations/:public_id/restore",
            patch(super::handlers::restore_organization)
                .layer(SetHandlerName::of(&super::handlers::restore_organization)),
        )
        .route(
            "/tenants/:public_id/suspend",
            post(super::handlers::suspend_tenant)
                .layer(SetHandlerName::of(&super::handlers::suspend_tenant)),
        )
        .route(
            "/tenants/:public_id/reactivate",
            post(super::handlers::reactivate_tenant)
                .layer(SetHandlerName::of(&super::handlers::reactivate_tenant)),
        )
        .with_state(state)
}
