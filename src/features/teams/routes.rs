use axum::{
    routing::{delete, get, patch},
    Router,
};

use crate::state::AppState;

pub fn routes(state: AppState) -> Router {
    Router::new()
        .route(
            "/teams",
            get(super::handlers::list_teams).post(super::handlers::create_team),
        )
        .route(
            "/teams/:public_id",
            get(super::handlers::get_team)
                .patch(super::handlers::update_team)
                .delete(super::handlers::delete_team),
        )
        .route(
            "/teams/:public_id/restore",
            patch(super::handlers::restore_team),
        )
        .route(
            "/teams/:public_id/members",
            patch(super::handlers::add_team_members),
        )
        .route(
            "/teams/:public_id/members/:member_public_id",
            delete(super::handlers::remove_team_member),
        )
        .with_state(state)
}
