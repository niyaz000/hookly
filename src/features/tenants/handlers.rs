use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{
        types::RequestContext,
        PublicUuid, ValidatedJson,
    },
    error::AppError,
    features::tenants::{
        models::{
            CreateTenantRequest, ListTenantsQuery, ListTenantsResponse, TenantResponse,
            UpdateTenantRequest,
        },
        repository::TenantRepository,
        service::TenantService,
    },
    state::AppState,
};

fn make_ctx() -> RequestContext {
    RequestContext {
        request_id: PublicUuid::new_v7().into_inner(),
        created_by: PublicUuid::new_v7().into_inner(),
    }
}

fn service(state: AppState) -> TenantService {
    TenantService::new(TenantRepository::new(state.db))
}

pub async fn create_tenant(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<CreateTenantRequest>,
) -> Result<(StatusCode, Json<TenantResponse>), AppError> {
    let tenant = service(state).create(payload, make_ctx()).await?;
    Ok((StatusCode::CREATED, Json(tenant)))
}

pub async fn get_tenant(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<TenantResponse>, AppError> {
    let tenant = service(state).get_by_public_id(public_id).await?;
    Ok(Json(tenant))
}

pub async fn update_tenant(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateTenantRequest>,
) -> Result<Json<TenantResponse>, AppError> {
    let tenant = service(state).update(public_id, payload, make_ctx()).await?;
    Ok(Json(tenant))
}

pub async fn delete_tenant(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    service(state).delete(public_id, make_ctx()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn suspend_tenant(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<TenantResponse>, AppError> {
    let tenant = service(state).suspend(public_id, make_ctx()).await?;
    Ok(Json(tenant))
}

pub async fn reactivate_tenant(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<TenantResponse>, AppError> {
    let tenant = service(state).reactivate(public_id, make_ctx()).await?;
    Ok(Json(tenant))
}

pub async fn list_tenants(
    State(state): State<AppState>,
    Query(query): Query<ListTenantsQuery>,
) -> Result<Json<ListTenantsResponse>, AppError> {
    let result = service(state).list(query).await?;
    Ok(Json(result))
}
