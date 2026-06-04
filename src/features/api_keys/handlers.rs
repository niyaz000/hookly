use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{types::RequestContext, qs_query::QsQuery, ValidatedJson},
    error::AppError,
    features::environments::repository::EnvironmentRepository,
    state::AppState,
};

use super::{
    models::{
        ApiKeyResponse, ApiKeySettingsResponse, CreateApiKeyRequest, ListApiKeysQuery,
        ListApiKeysResponse, RevealApiKeyResponse, UpdateApiKeyRequest, UpdateApiKeySettingsRequest,
        UpsertApiKeySettingsRequest,
    },
    repository::ApiKeyRepository,
    service::ApiKeyService,
};

fn make_svc(state: AppState) -> ApiKeyService {
    ApiKeyService::new(
        ApiKeyRepository::new(state.db.clone()),
        EnvironmentRepository::new(state.db),
        state.key_provider,
    )
}

pub async fn create_api_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    ValidatedJson(payload): ValidatedJson<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<ApiKeyResponse>), AppError> {
    let payload = payload.normalize();
    payload.validate_all()?;

    let svc = make_svc(state);
    let (key, plaintext) = svc.create(payload, ctx).await?;

    let mut resp = ApiKeyResponse::from(key);
    resp.key = Some(plaintext);

    Ok((StatusCode::CREATED, Json(resp)))
}

pub async fn list_api_keys(
    State(state): State<AppState>,
    QsQuery(query): QsQuery<ListApiKeysQuery>,
) -> Result<(StatusCode, Json<ListApiKeysResponse>), AppError> {
    let tenant_id = query
        .tenant_id
        .ok_or_else(|| AppError::BadRequest("tenant_id query parameter is required".into()))?;

    let svc = make_svc(state);
    let resp = svc.list(tenant_id, query).await?;

    Ok((StatusCode::OK, Json(resp)))
}

pub async fn get_api_key(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<ApiKeyResponse>), AppError> {
    let svc = make_svc(state);
    let key = svc.get_by_id(&public_id).await?;

    Ok((StatusCode::OK, Json(ApiKeyResponse::from(key))))
}

pub async fn update_api_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateApiKeyRequest>,
) -> Result<(StatusCode, Json<ApiKeyResponse>), AppError> {
    let payload = payload.normalize();
    payload.validate_all()?;

    let svc = make_svc(state);
    let key = svc.update(&public_id, payload, ctx).await?;

    Ok((StatusCode::OK, Json(ApiKeyResponse::from(key))))
}

pub async fn delete_api_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let svc = make_svc(state);
    svc.delete(&public_id, ctx).await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn reveal_api_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<RevealApiKeyResponse>), AppError> {
    let svc = make_svc(state);
    let resp = svc.reveal(&public_id, ctx).await?;

    Ok((StatusCode::OK, Json(resp)))
}

pub async fn upsert_api_key_settings(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    ValidatedJson(payload): ValidatedJson<UpsertApiKeySettingsRequest>,
) -> Result<(StatusCode, Json<ApiKeySettingsResponse>), AppError> {
    payload.validate_all()?;

    let svc = make_svc(state);
    let settings = svc.upsert_settings(payload, ctx).await?;

    Ok((StatusCode::CREATED, Json(ApiKeySettingsResponse::from(settings))))
}

pub async fn get_api_key_settings(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<ApiKeySettingsResponse>), AppError> {
    let svc = make_svc(state);
    let settings = svc.get_settings_by_id(&public_id).await?;

    Ok((StatusCode::OK, Json(ApiKeySettingsResponse::from(settings))))
}

pub async fn update_api_key_settings(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateApiKeySettingsRequest>,
) -> Result<(StatusCode, Json<ApiKeySettingsResponse>), AppError> {
    payload.validate_all()?;

    let svc = make_svc(state);
    let settings = svc.update_settings(&public_id, payload, ctx).await?;

    Ok((StatusCode::OK, Json(ApiKeySettingsResponse::from(settings))))
}
