use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{qs_query::QsQuery, types::RequestContext, validators, ValidatedJson},
    error::AppError,
    features::permissions::repository::PermissionRepository,
    state::AppState,
};

use super::{
    models::{
        AssignPermissionsRequest, AssignPermissionsResponse, CreateRoleRequest,
        ListRolePermissionsResponse, ListRolesQuery, ListRolesResponse, RoleResponse,
        UpdateRoleRequest,
    },
    repository::RoleRepository,
    service::RoleService,
};

fn make_svc(state: AppState) -> RoleService {
    RoleService::new(
        RoleRepository::new(state.db.clone()),
        PermissionRepository::new(state.db),
    )
}

pub async fn create_role(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    ValidatedJson(payload): ValidatedJson<CreateRoleRequest>,
) -> Result<(StatusCode, Json<RoleResponse>), AppError> {
    let payload = payload.normalize();
    payload.validate_all()?;

    let role = make_svc(state).create(payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(RoleResponse::from(role))))
}

pub async fn get_role(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<RoleResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "rol_", "role")?;
    let role = make_svc(state).get_by_id(&public_id).await?;
    Ok(Json(RoleResponse::from(role)))
}

pub async fn list_roles(
    State(state): State<AppState>,
    QsQuery(query): QsQuery<ListRolesQuery>,
) -> Result<Json<ListRolesResponse>, AppError> {
    let tenant_id = query.tenant_id;
    let resp = make_svc(state).list(tenant_id, query).await?;
    Ok(Json(resp))
}

pub async fn update_role(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateRoleRequest>,
) -> Result<Json<RoleResponse>, AppError> {
    let payload = payload.normalize();
    let role = make_svc(state).update(&public_id, payload, ctx).await?;
    Ok(Json(RoleResponse::from(role)))
}

pub async fn delete_role(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    make_svc(state).delete(&public_id, ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_role_permissions(
    State(state): State<AppState>,
    Path(role_id): Path<String>,
) -> Result<Json<ListRolePermissionsResponse>, AppError> {
    let resp = make_svc(state).list_permissions(&role_id).await?;
    Ok(Json(resp))
}

pub async fn assign_role_permissions(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(role_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<AssignPermissionsRequest>,
) -> Result<Json<AssignPermissionsResponse>, AppError> {
    let resp = make_svc(state).assign_permissions(&role_id, payload, ctx).await?;
    Ok(Json(resp))
}

pub async fn remove_role_permission(
    State(state): State<AppState>,
    Path((role_id, perm_id)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    make_svc(state).remove_permission(&role_id, &perm_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
