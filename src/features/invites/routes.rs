use axum::{
    routing::{delete, get, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/invites", get(super::handlers::list_invites)
            .layer(SetHandlerName::of(&super::handlers::list_invites)))
        .route("/invites", post(super::handlers::create_invite)
            .layer(SetHandlerName::of(&super::handlers::create_invite)))
        // static action routes registered before /:public_id so they take precedence
        .route("/invites/verify", post(super::handlers::verify_invite)
            .layer(SetHandlerName::of(&super::handlers::verify_invite)))
        .route("/invites/accept", post(super::handlers::accept_invite)
            .layer(SetHandlerName::of(&super::handlers::accept_invite)))
        .route("/invites/:public_id", get(super::handlers::get_invite)
            .layer(SetHandlerName::of(&super::handlers::get_invite)))
        .route("/invites/:public_id", delete(super::handlers::delete_invite)
            .layer(SetHandlerName::of(&super::handlers::delete_invite)))
        .route("/invites/:public_id/revoke", post(super::handlers::revoke_invite)
            .layer(SetHandlerName::of(&super::handlers::revoke_invite)))
        .route("/invites/:public_id/resend", post(super::handlers::resend_invite)
            .layer(SetHandlerName::of(&super::handlers::resend_invite)))
        .with_state(state)
}
