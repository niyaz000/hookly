use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{qs_query::QsQuery, types::RequestContext, validators, ValidatedJson},
    error::AppError,
    features::{
        permissions::repository::PermissionRepository,
        roles::repository::RoleRepository,
        tenants::{
            models::{
                CreateTenantRequest, ListTenantsQuery, ListTenantsResponse, TenantResponse,
                UpdateTenantRequest,
            },
            repository::TenantRepository,
            service::TenantService,
        },
    },
    state::AppState,
};

fn service(state: AppState) -> TenantService {
    TenantService::new(
        TenantRepository::new(state.db.clone()),
        RoleRepository::new(state.db.clone()),
        PermissionRepository::new(state.db),
    )
}

pub async fn create_tenant(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    ValidatedJson(payload): ValidatedJson<CreateTenantRequest>,
) -> Result<(StatusCode, Json<TenantResponse>), AppError> {
    let tenant = service(state).create(payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(tenant)))
}

pub async fn get_tenant(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<TenantResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "ten_", "tenant")?;
    let tenant = service(state).get_by_public_id(public_id).await?;
    Ok(Json(tenant))
}

pub async fn update_tenant(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateTenantRequest>,
) -> Result<Json<TenantResponse>, AppError> {
    let tenant = service(state).update(public_id, payload, ctx).await?;
    Ok(Json(tenant))
}

pub async fn delete_tenant(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    service(state).delete(public_id, ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn suspend_tenant(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<TenantResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "ten_", "tenant")?;
    // Scope the suspend to the caller's organization — prevents cross-org suspension.
    let tenant = service(state)
        .suspend(public_id, Some(ctx.organization_id), ctx)
        .await?;
    Ok(Json(tenant))
}

pub async fn reactivate_tenant(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<TenantResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "ten_", "tenant")?;
    // Scope the reactivate to the caller's organization — prevents cross-org changes.
    let tenant = service(state)
        .reactivate(public_id, Some(ctx.organization_id), ctx)
        .await?;
    Ok(Json(tenant))
}

pub async fn list_tenants(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    QsQuery(query): QsQuery<ListTenantsQuery>,
) -> Result<Json<ListTenantsResponse>, AppError> {
    let result = service(state).list(query, ctx).await?;
    Ok(Json(result))
}
