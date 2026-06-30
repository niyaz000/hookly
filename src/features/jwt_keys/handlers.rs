use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::{qs_query::QsQuery, types::RequestContext, validators, ValidatedJson},
    error::AppError,
    state::AppState,
};

use super::{
    models::{
        CreateJwtKeyRequest, GenerateKeyPairRequest, GenerateKeyPairResponse, JwtKeyResponse,
        JwksResponse, ListJwtKeysQuery, ListJwtKeysResponse, RotateJwtKeyRequest,
        UpdateJwtKeyRequest,
    },
    repository::JwtKeyRepository,
    service::JwtKeyService,
};

fn make_svc(state: AppState) -> JwtKeyService {
    JwtKeyService::new(JwtKeyRepository::new(state.db), state.crypto)
}

pub async fn create_jwt_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    ValidatedJson(payload): ValidatedJson<CreateJwtKeyRequest>,
) -> Result<(StatusCode, Json<JwtKeyResponse>), AppError> {
    let payload = payload.normalize();
    payload.validate_all()?;

    let key = make_svc(state).create(payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(key)))
}

pub async fn get_jwt_key(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<JwtKeyResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "jwk_", "jwt key")?;
    let key = make_svc(state).get_by_id(&public_id).await?;
    Ok(Json(JwtKeyResponse::from_key(key)))
}

pub async fn list_jwt_keys(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    QsQuery(query): QsQuery<ListJwtKeysQuery>,
) -> Result<Json<ListJwtKeysResponse>, AppError> {
    let resp = make_svc(state).list(query, ctx).await?;
    Ok(Json(resp))
}

pub async fn update_jwt_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateJwtKeyRequest>,
) -> Result<Json<JwtKeyResponse>, AppError> {
    let payload = payload.normalize();
    let key = make_svc(state).update(&public_id, payload, ctx).await?;
    Ok(Json(key))
}

pub async fn rotate_jwt_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<RotateJwtKeyRequest>,
) -> Result<(StatusCode, Json<JwtKeyResponse>), AppError> {
    let key = make_svc(state).rotate(&public_id, payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(key)))
}

pub async fn enable_jwt_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<JwtKeyResponse>, AppError> {
    let key = make_svc(state).enable(&public_id, ctx).await?;
    Ok(Json(key))
}

pub async fn disable_jwt_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<JwtKeyResponse>, AppError> {
    let key = make_svc(state).disable(&public_id, ctx).await?;
    Ok(Json(key))
}

pub async fn delete_jwt_key(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    make_svc(state).delete(&public_id, ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_public_key(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<String, AppError> {
    let key = make_svc(state).get_by_id(&public_id).await?;
    key.public_key
        .ok_or_else(|| AppError::BadRequest("this key has no public key (HMAC algorithm)".into()))
}

#[derive(Debug, Deserialize)]
pub struct JwksQuery {
    pub tenant_id: Uuid,
}

pub async fn get_jwks(
    State(state): State<AppState>,
    Query(q): Query<JwksQuery>,
) -> Result<Json<JwksResponse>, AppError> {
    let resp = make_svc(state).get_jwks(q.tenant_id).await?;
    Ok(Json(resp))
}

pub async fn generate_key_pair(
    ValidatedJson(payload): ValidatedJson<GenerateKeyPairRequest>,
) -> Result<Json<GenerateKeyPairResponse>, AppError> {
    let resp = JwtKeyService::generate_ephemeral(payload)?;
    Ok(Json(resp))
}
