use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/tenants",
            get(super::handlers::list_tenants).post(super::handlers::create_tenant),
        )
        .route(
            "/tenants/:public_id",
            get(super::handlers::get_tenant)
                .patch(super::handlers::update_tenant)
                .delete(super::handlers::delete_tenant),
        )
        .route(
            "/tenants/:public_id/suspend",
            post(super::handlers::suspend_tenant),
        )
        .route(
            "/tenants/:public_id/reactivate",
            post(super::handlers::reactivate_tenant),
        )
        .with_state(state)
}
