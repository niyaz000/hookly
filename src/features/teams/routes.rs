use axum::{
    routing::{delete, get, patch, post},
    Router,
};

use crate::common::SetHandlerName;
use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route("/teams", get(super::handlers::list_teams)
            .layer(SetHandlerName::of(&super::handlers::list_teams)))
        .route("/teams", post(super::handlers::create_team)
            .layer(SetHandlerName::of(&super::handlers::create_team)))
        .route("/teams/:public_id", get(super::handlers::get_team)
            .layer(SetHandlerName::of(&super::handlers::get_team)))
        .route("/teams/:public_id", patch(super::handlers::update_team)
            .layer(SetHandlerName::of(&super::handlers::update_team)))
        .route("/teams/:public_id", delete(super::handlers::delete_team)
            .layer(SetHandlerName::of(&super::handlers::delete_team)))
        .route("/teams/:public_id/restore", patch(super::handlers::restore_team)
            .layer(SetHandlerName::of(&super::handlers::restore_team)))
        .route("/teams/:public_id/members", patch(super::handlers::add_team_members)
            .layer(SetHandlerName::of(&super::handlers::add_team_members)))
        .route("/teams/:public_id/members/:member_public_id", delete(super::handlers::remove_team_member)
            .layer(SetHandlerName::of(&super::handlers::remove_team_member)))
        .with_state(state)
}
