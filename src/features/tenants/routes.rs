use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/tenants", get(super::handlers::list_tenants)
            .layer(SetHandlerName::of(&super::handlers::list_tenants)))
        .route("/tenants", post(super::handlers::create_tenant)
            .layer(SetHandlerName::of(&super::handlers::create_tenant)))
        .route("/tenants/:public_id", get(super::handlers::get_tenant)
            .layer(SetHandlerName::of(&super::handlers::get_tenant)))
        .route("/tenants/:public_id", patch(super::handlers::update_tenant)
            .layer(SetHandlerName::of(&super::handlers::update_tenant)))
        .route("/tenants/:public_id", delete(super::handlers::delete_tenant)
            .layer(SetHandlerName::of(&super::handlers::delete_tenant)))
        .route("/tenants/:public_id/suspend", post(super::handlers::suspend_tenant)
            .layer(SetHandlerName::of(&super::handlers::suspend_tenant)))
        .route("/tenants/:public_id/reactivate", post(super::handlers::reactivate_tenant)
            .layer(SetHandlerName::of(&super::handlers::reactivate_tenant)))
        .with_state(state)
}
