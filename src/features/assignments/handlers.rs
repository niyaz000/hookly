use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    common::{types::RequestContext, ValidatedJson},
    error::AppError,
    features::{
        assignments::repository::AssignmentRepository,
        permissions::repository::PermissionRepository,
        roles::repository::RoleRepository,
    },
    state::AppState,
};

use super::{
    models::{
        AssignPermissionsRequest, AssignRolesRequest, BulkAssignResponse,
        EffectivePermissionsResponse, ListAssignedPermissionsResponse, ListAssignedRolesResponse,
    },
    service::AssignmentService,
};

fn make_svc(state: AppState) -> AssignmentService {
    AssignmentService::new(
        AssignmentRepository::new(state.db.clone()),
        RoleRepository::new(state.db.clone()),
        PermissionRepository::new(state.db),
    )
}

#[derive(Debug, Deserialize)]
pub struct TenantQuery {
    pub tenant_id: Uuid,
}

// ── User roles ─────────────────────────────────────────────────────────────────

pub async fn list_user_roles(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<ListAssignedRolesResponse>, AppError> {
    let resp = make_svc(state).list_user_roles(&user_id).await?;
    Ok(Json(resp))
}

pub async fn assign_user_roles(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(user_id): Path<String>,
    Query(q): Query<TenantQuery>,
    ValidatedJson(payload): ValidatedJson<AssignRolesRequest>,
) -> Result<Json<BulkAssignResponse>, AppError> {
    let resp = make_svc(state).assign_user_roles(&user_id, q.tenant_id, payload, ctx).await?;
    Ok(Json(resp))
}

pub async fn remove_user_role(
    State(state): State<AppState>,
    Path((user_id, role_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    make_svc(state).remove_user_role(&user_id, &role_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── User permissions ───────────────────────────────────────────────────────────

pub async fn list_user_permissions(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> Result<Json<ListAssignedPermissionsResponse>, AppError> {
    let resp = make_svc(state).list_user_permissions(&user_id).await?;
    Ok(Json(resp))
}

pub async fn assign_user_permissions(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(user_id): Path<String>,
    Query(q): Query<TenantQuery>,
    ValidatedJson(payload): ValidatedJson<AssignPermissionsRequest>,
) -> Result<Json<BulkAssignResponse>, AppError> {
    let resp =
        make_svc(state).assign_user_permissions(&user_id, q.tenant_id, payload, ctx).await?;
    Ok(Json(resp))
}

pub async fn remove_user_permission(
    State(state): State<AppState>,
    Path((user_id, perm_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    make_svc(state).remove_user_permission(&user_id, &perm_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_user_effective_permissions(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Query(q): Query<TenantQuery>,
) -> Result<Json<EffectivePermissionsResponse>, AppError> {
    let resp =
        make_svc(state).get_user_effective_permissions(&user_id, q.tenant_id).await?;
    Ok(Json(resp))
}

// ── API key roles ─────────────────────────────────────────────────────────────

pub async fn list_api_key_roles(
    State(state): State<AppState>,
    Path(api_key_id): Path<String>,
) -> Result<Json<ListAssignedRolesResponse>, AppError> {
    let resp = make_svc(state).list_api_key_roles(&api_key_id).await?;
    Ok(Json(resp))
}

pub async fn assign_api_key_roles(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(api_key_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<AssignRolesRequest>,
) -> Result<Json<BulkAssignResponse>, AppError> {
    let resp = make_svc(state).assign_api_key_roles(&api_key_id, payload, ctx).await?;
    Ok(Json(resp))
}

pub async fn remove_api_key_role(
    State(state): State<AppState>,
    Path((api_key_id, role_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    make_svc(state).remove_api_key_role(&api_key_id, &role_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── API key permissions ───────────────────────────────────────────────────────

pub async fn list_api_key_permissions(
    State(state): State<AppState>,
    Path(api_key_id): Path<String>,
) -> Result<Json<ListAssignedPermissionsResponse>, AppError> {
    let resp = make_svc(state).list_api_key_permissions(&api_key_id).await?;
    Ok(Json(resp))
}

pub async fn assign_api_key_permissions(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(api_key_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<AssignPermissionsRequest>,
) -> Result<Json<BulkAssignResponse>, AppError> {
    let resp = make_svc(state).assign_api_key_permissions(&api_key_id, payload, ctx).await?;
    Ok(Json(resp))
}

pub async fn remove_api_key_permission(
    State(state): State<AppState>,
    Path((api_key_id, perm_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    make_svc(state).remove_api_key_permission(&api_key_id, &perm_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
