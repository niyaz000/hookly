use axum::{
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::{
    common::{
        idempotency,
        qs_query::QsQuery,
        types::RequestContext,
        validators,
        ValidatedJson,
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

fn service(state: AppState) -> TeamService {
    TeamService::new(TeamRepository::new(state.db))
}

pub async fn create_team(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<CreateTeamRequest>,
) -> Result<(StatusCode, Json<TeamResponse>), AppError> {
    if let Some(key) = idempotency::extract_key(&headers)? {
        let hash = idempotency::body_hash(&payload);
        let redis = state.redis.clone();
        let team = idempotency::resolve(
            &redis,
            "teams",
            &key,
            &hash,
            move || async move { service(state).create(payload, ctx).await },
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(team)));
    }

    let team = service(state).create(payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(team)))
}

pub async fn get_team(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<TeamResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "tea_", "team")?;
    let team = service(state).get_by_public_id(public_id).await?;
    Ok(Json(team))
}

pub async fn update_team(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateTeamRequest>,
) -> Result<Json<TeamResponse>, AppError> {
    let team = service(state).update(public_id, payload, ctx).await?;
    Ok(Json(team))
}

pub async fn delete_team(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    service(state).delete(public_id, ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore_team(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<TeamResponse>, AppError> {
    let team = service(state).restore(public_id, ctx).await?;
    Ok(Json(team))
}

pub async fn list_teams(
    State(state): State<AppState>,
    QsQuery(query): QsQuery<ListTeamsQuery>,
) -> Result<Json<ListTeamsResponse>, AppError> {
    let result = service(state).list(query).await?;
    Ok(Json(result))
}

pub async fn add_team_members(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<AddTeamMembersRequest>,
) -> Result<Json<Vec<TeamMemberResponse>>, AppError> {
    if let Some(key) = idempotency::extract_key(&headers)? {
        let hash = idempotency::body_hash(&payload);
        let redis = state.redis.clone();
        let ns = format!("team_members:{}", public_id);
        let members = idempotency::resolve(
            &redis,
            &ns,
            &key,
            &hash,
            move || async move {
                service(state).add_members(public_id, payload, ctx).await
            },
        )
        .await?;
        return Ok(Json(members));
    }

    let members = service(state).add_members(public_id, payload, ctx).await?;
    Ok(Json(members))
}

pub async fn remove_team_member(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path((team_public_id, member_public_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    service(state)
        .remove_member(team_public_id, member_public_id, ctx)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
