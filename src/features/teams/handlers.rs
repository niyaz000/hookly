use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{
        types::RequestContext,
        PublicUuid, ValidatedJson,
    },
    error::AppError,
    features::teams::{
        models::{
            AddTeamMembersRequest, CreateTeamRequest, ListTeamsQuery, ListTeamsResponse,
            TeamMemberResponse, TeamResponse, UpdateTeamRequest,
        },
        repository::TeamRepository,
        service::TeamService,
    },
    state::AppState,
};

fn make_ctx() -> RequestContext {
    RequestContext {
        request_id: PublicUuid::new_v7().into_inner(),
        created_by: PublicUuid::new_v7().into_inner(),
    }
}

fn service(state: AppState) -> TeamService {
    TeamService::new(TeamRepository::new(state.db))
}

pub async fn create_team(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<CreateTeamRequest>,
) -> Result<(StatusCode, Json<TeamResponse>), AppError> {
    let team = service(state).create(payload, make_ctx()).await?;
    Ok((StatusCode::CREATED, Json(team)))
}

pub async fn get_team(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<TeamResponse>, AppError> {
    let team = service(state).get_by_public_id(public_id).await?;
    Ok(Json(team))
}

pub async fn update_team(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateTeamRequest>,
) -> Result<Json<TeamResponse>, AppError> {
    let team = service(state).update(public_id, payload, make_ctx()).await?;
    Ok(Json(team))
}

pub async fn delete_team(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    service(state).delete(public_id, make_ctx()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore_team(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<TeamResponse>, AppError> {
    let team = service(state).restore(public_id, make_ctx()).await?;
    Ok(Json(team))
}

pub async fn list_teams(
    State(state): State<AppState>,
    Query(query): Query<ListTeamsQuery>,
) -> Result<Json<ListTeamsResponse>, AppError> {
    let result = service(state).list(query).await?;
    Ok(Json(result))
}

pub async fn add_team_members(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<AddTeamMembersRequest>,
) -> Result<Json<Vec<TeamMemberResponse>>, AppError> {
    let members = service(state).add_members(public_id, payload, make_ctx()).await?;
    Ok(Json(members))
}

pub async fn remove_team_member(
    State(state): State<AppState>,
    Path((team_public_id, member_public_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    service(state)
        .remove_member(team_public_id, member_public_id, make_ctx())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
