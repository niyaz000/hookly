use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{qs_query::QsQuery, validators, ValidatedJson},
    error::AppError,
    state::AppState,
};

use super::{
    models::{
        CreatePermissionRequest, ListPermissionsQuery, ListPermissionsResponse,
        PermissionResponse, UpdatePermissionRequest,
    },
    repository::PermissionRepository,
    service::PermissionService,
};

fn make_svc(state: AppState) -> PermissionService {
    PermissionService::new(PermissionRepository::new(state.db))
}

pub async fn create_permission(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<CreatePermissionRequest>,
) -> Result<(StatusCode, Json<PermissionResponse>), AppError> {
    let payload = payload.normalize();
    payload.validate_all()?;

    let perm = make_svc(state).create(payload).await?;
    Ok((StatusCode::CREATED, Json(PermissionResponse::from(perm))))
}

pub async fn get_permission(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<PermissionResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "per_", "permission")?;
    let perm = make_svc(state).get_by_id(&public_id).await?;
    Ok(Json(PermissionResponse::from(perm)))
}

pub async fn list_permissions(
    State(state): State<AppState>,
    QsQuery(query): QsQuery<ListPermissionsQuery>,
) -> Result<Json<ListPermissionsResponse>, AppError> {
    let tenant_id = query.tenant_id;
    let resp = make_svc(state).list(tenant_id, query).await?;
    Ok(Json(resp))
}

pub async fn update_permission(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdatePermissionRequest>,
) -> Result<Json<PermissionResponse>, AppError> {
    let payload = payload.normalize();
    let perm = make_svc(state).update(&public_id, payload).await?;
    Ok(Json(PermissionResponse::from(perm)))
}

pub async fn delete_permission(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    make_svc(state).delete(&public_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
