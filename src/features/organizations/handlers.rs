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
    features::organizations::{
        models::{
            CreateOrganizationRequest, ListOrganizationsQuery, ListOrganizationsResponse,
            OrganizationResponse, UpdateOrganizationRequest,
        },
        repository::OrganizationRepository,
        service::OrganizationService,
    },
    state::AppState,
};

fn make_ctx() -> RequestContext {
    RequestContext {
        request_id: PublicUuid::new_v7().into_inner(),
        created_by: PublicUuid::new_v7().into_inner(),
    }
}

fn service(state: AppState) -> OrganizationService {
    OrganizationService::new(OrganizationRepository::new(state.db))
}

pub async fn create_organization(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<OrganizationResponse>), AppError> {
    let org = service(state).create(payload, make_ctx()).await?;
    Ok((StatusCode::CREATED, Json(org)))
}

pub async fn get_organization(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<OrganizationResponse>, AppError> {
    let org = service(state).get_by_public_id(public_id).await?;
    Ok(Json(org))
}

pub async fn update_organization(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateOrganizationRequest>,
) -> Result<Json<OrganizationResponse>, AppError> {
    let org = service(state).update(public_id, payload, make_ctx()).await?;
    Ok(Json(org))
}

pub async fn delete_organization(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    service(state).delete(public_id, make_ctx()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn suspend_organization(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<OrganizationResponse>, AppError> {
    let org = service(state).suspend(public_id, make_ctx()).await?;
    Ok(Json(org))
}

pub async fn restore_organization(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<OrganizationResponse>, AppError> {
    let org = service(state).restore(public_id, make_ctx()).await?;
    Ok(Json(org))
}

pub async fn list_organizations(
    State(state): State<AppState>,
    Query(query): Query<ListOrganizationsQuery>,
) -> Result<Json<ListOrganizationsResponse>, AppError> {
    let result = service(state).list(query).await?;
    Ok(Json(result))
}
