use axum::{
    routing::{get, post},
    Router,
};

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/invites",
            get(super::handlers::list_invites).post(super::handlers::create_invite),
        )
        // static action routes registered before /:public_id so they take precedence
        .route("/invites/verify", post(super::handlers::verify_invite))
        .route("/invites/accept", post(super::handlers::accept_invite))
        .route(
            "/invites/:public_id",
            get(super::handlers::get_invite).delete(super::handlers::delete_invite),
        )
        .route(
            "/invites/:public_id/revoke",
            post(super::handlers::revoke_invite),
        )
        .route(
            "/invites/:public_id/resend",
            post(super::handlers::resend_invite),
        )
        .with_state(state)
}
