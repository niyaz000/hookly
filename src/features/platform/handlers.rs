use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{types::RequestContext, validators, ValidatedJson},
    error::AppError,
    features::{
        organizations::models::CreateOrganizationRequest,
        permissions::repository::PermissionRepository,
        roles::repository::RoleRepository,
        tenants::{
            models::TenantResponse,
            repository::TenantRepository,
            service::TenantService,
        },
        organizations::{
            models::OrganizationResponse,
            repository::OrganizationRepository,
            service::OrganizationService,
        },
    },
    state::AppState,
};

use super::service::{BootstrapResponse, BootstrapService};

fn bootstrap_service(state: AppState) -> BootstrapService {
    BootstrapService::new(
        state.db.clone(),
        RoleRepository::new(state.db.clone()),
        PermissionRepository::new(state.db),
    )
}

fn org_service(state: AppState) -> OrganizationService {
    OrganizationService::new(OrganizationRepository::new(state.db))
}

fn tenant_service(state: AppState) -> TenantService {
    TenantService::new(
        TenantRepository::new(state.db.clone()),
        RoleRepository::new(state.db.clone()),
        PermissionRepository::new(state.db),
    )
}

pub async fn create_organization(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    ValidatedJson(payload): ValidatedJson<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<BootstrapResponse>), AppError> {
    let result = bootstrap_service(state).bootstrap_organization(payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(result)))
}

pub async fn restore_organization(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<OrganizationResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "org_", "organization")?;
    let org = org_service(state).restore(public_id, ctx).await?;
    Ok(Json(org))
}

pub async fn suspend_tenant(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<TenantResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "ten_", "tenant")?;
    // None = admin bypass, no org ownership filter
    let tenant = tenant_service(state).suspend(public_id, None, ctx).await?;
    Ok(Json(tenant))
}

pub async fn reactivate_tenant(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<TenantResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "ten_", "tenant")?;
    // None = admin bypass, no org ownership filter
    let tenant = tenant_service(state).reactivate(public_id, None, ctx).await?;
    Ok(Json(tenant))
}
