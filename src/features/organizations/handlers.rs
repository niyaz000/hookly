use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{qs_query::QsQuery, types::RequestContext, validators, ValidatedJson},
    error::AppError,
    features::organizations::{
        models::{
            ListOrganizationsQuery, ListOrganizationsResponse,
            OrganizationResponse, UpdateOrganizationRequest,
        },
        repository::OrganizationRepository,
        service::OrganizationService,
    },
    state::AppState,
};

fn service(state: AppState) -> OrganizationService {
    OrganizationService::new(OrganizationRepository::new(state.db))
}

pub async fn get_organization(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<OrganizationResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "org_", "organization")?;
    let org = service(state).get_by_public_id(public_id).await?;
    Ok(Json(org))
}

pub async fn update_organization(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateOrganizationRequest>,
) -> Result<Json<OrganizationResponse>, AppError> {
    let org = service(state).update(public_id, payload, ctx).await?;
    Ok(Json(org))
}

pub async fn delete_organization(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    service(state).delete(public_id, ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn suspend_organization(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<OrganizationResponse>, AppError> {
    validators::validate_id_prefix(&public_id, "org_", "organization")?;
    // Scope the suspend to the caller's own org — prevents suspending someone else's org.
    let org = service(state)
        .suspend(public_id, Some(ctx.organization_id), ctx)
        .await?;
    Ok(Json(org))
}

pub async fn list_organizations(
    State(state): State<AppState>,
    QsQuery(query): QsQuery<ListOrganizationsQuery>,
) -> Result<Json<ListOrganizationsResponse>, AppError> {
    let result = service(state).list(query).await?;
    Ok(Json(result))
}
