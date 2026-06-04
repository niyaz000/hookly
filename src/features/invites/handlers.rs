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
        ValidatedJson,
    },
    error::AppError,
    features::invites::{
        models::{
            AcceptInviteRequest, CreateInviteRequest, InviteResponse, InviteVerifyResponse,
            ListInvitesQuery, ListInvitesResponse, TenantMemberResponse, VerifyInviteRequest,
        },
        repository::InviteRepository,
        service::InviteService,
    },
    state::AppState,
};

fn service(state: &AppState) -> InviteService {
    InviteService::new(InviteRepository::new(state.db.clone()))
}

pub async fn create_invite(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<CreateInviteRequest>,
) -> Result<(StatusCode, Json<InviteResponse>), AppError> {
    if let Some(key) = idempotency::extract_key(&headers)? {
        let hash = idempotency::body_hash(&payload);
        let redis = state.redis.clone();
        let invite = idempotency::resolve(
            &redis,
            "invites",
            &key,
            &hash,
            move || async move {
                service(&state).create(payload, ctx.request_id, state.email.as_ref()).await
            },
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(invite)));
    }

    let invite = service(&state).create(payload, ctx.request_id, state.email.as_ref()).await?;
    Ok((StatusCode::CREATED, Json(invite)))
}

pub async fn get_invite(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<InviteResponse>, AppError> {
    let invite = service(&state).get(public_id).await?;
    Ok(Json(invite))
}

pub async fn list_invites(
    State(state): State<AppState>,
    QsQuery(query): QsQuery<ListInvitesQuery>,
) -> Result<Json<ListInvitesResponse>, AppError> {
    let result = service(&state).list(query).await?;
    Ok(Json(result))
}

pub async fn delete_invite(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    service(&state).delete(public_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn revoke_invite(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<InviteResponse>, AppError> {
    let invite = service(&state).revoke(public_id).await?;
    Ok(Json(invite))
}

pub async fn resend_invite(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<InviteResponse>, AppError> {
    let invite = service(&state)
        .resend(public_id, ctx.request_id, state.email.as_ref())
        .await?;
    Ok(Json(invite))
}

pub async fn verify_invite(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<VerifyInviteRequest>,
) -> Result<Json<InviteVerifyResponse>, AppError> {
    let result = service(&state).verify(payload).await?;
    Ok(Json(result))
}

pub async fn accept_invite(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<AcceptInviteRequest>,
) -> Result<Json<TenantMemberResponse>, AppError> {
    let member = service(&state).accept(payload).await?;
    Ok(Json(member))
}
