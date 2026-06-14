use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{qs_query::QsQuery, types::RequestContext, validators, ValidatedJson},
    error::AppError,
    state::AppState,
};

use super::{
    models::{
        CreatePlatformWebhookRequest, ListPlatformWebhooksQuery, ListPlatformWebhooksResponse,
        PlatformWebhookResponse, UpdatePlatformWebhookRequest,
    },
    repository::PlatformWebhookRepository,
    service::PlatformWebhookService,
};

fn svc(state: AppState) -> PlatformWebhookService {
    PlatformWebhookService::new(PlatformWebhookRepository::new(state.db), state.crypto)
}

pub async fn create_platform_webhook(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    ValidatedJson(payload): ValidatedJson<CreatePlatformWebhookRequest>,
) -> Result<(StatusCode, Json<PlatformWebhookResponse>), AppError> {
    let payload = payload.normalize();
    payload.validate_all()?;
    let webhook = svc(state).create(payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(webhook)))
}

pub async fn list_platform_webhooks(
    State(state): State<AppState>,
    QsQuery(query): QsQuery<ListPlatformWebhooksQuery>,
) -> Result<(StatusCode, Json<ListPlatformWebhooksResponse>), AppError> {
    let resp = svc(state).list(query).await?;
    Ok((StatusCode::OK, Json(resp)))
}

pub async fn get_platform_webhook(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<PlatformWebhookResponse>), AppError> {
    validators::validate_id_prefix(&public_id, "pwh_", "platform webhook")?;
    let webhook = svc(state).get_by_id(&public_id).await?;
    Ok((StatusCode::OK, Json(webhook)))
}

pub async fn update_platform_webhook(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdatePlatformWebhookRequest>,
) -> Result<(StatusCode, Json<PlatformWebhookResponse>), AppError> {
    let payload = payload.normalize();
    payload.validate_all()?;
    let webhook = svc(state).update(&public_id, payload, ctx).await?;
    Ok((StatusCode::OK, Json(webhook)))
}

pub async fn delete_platform_webhook(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    svc(state).delete(&public_id, ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn suspend_platform_webhook(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<PlatformWebhookResponse>), AppError> {
    let webhook = svc(state).suspend(&public_id, ctx).await?;
    Ok((StatusCode::OK, Json(webhook)))
}

pub async fn activate_platform_webhook(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<PlatformWebhookResponse>), AppError> {
    let webhook = svc(state).activate(&public_id, ctx).await?;
    Ok((StatusCode::OK, Json(webhook)))
}

pub async fn rotate_platform_webhook_secret(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<PlatformWebhookResponse>), AppError> {
    let webhook = svc(state).rotate_secret(&public_id, ctx).await?;
    Ok((StatusCode::OK, Json(webhook)))
}
