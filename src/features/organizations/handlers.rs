use axum::{
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::{
    common::{
        idempotency,
        qs_query::QsQuery,
        types::RequestContext,
        ValidatedJson,
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

fn service(state: AppState) -> OrganizationService {
    OrganizationService::new(OrganizationRepository::new(state.db))
}

pub async fn create_organization(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<OrganizationResponse>), AppError> {
    if let Some(key) = idempotency::extract_key(&headers)? {
        let hash = idempotency::body_hash(&payload);
        let redis = state.redis.clone();
        let org = idempotency::resolve(
            &redis,
            "organizations",
            &key,
            &hash,
            move || async move { service(state).create(payload, ctx).await },
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(org)));
    }

    let org = service(state).create(payload, ctx).await?;
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
    let org = service(state).suspend(public_id, ctx).await?;
    Ok(Json(org))
}

pub async fn restore_organization(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<OrganizationResponse>, AppError> {
    let org = service(state).restore(public_id, ctx).await?;
    Ok(Json(org))
}

pub async fn list_organizations(
    State(state): State<AppState>,
    QsQuery(query): QsQuery<ListOrganizationsQuery>,
) -> Result<Json<ListOrganizationsResponse>, AppError> {
    let result = service(state).list(query).await?;
    Ok(Json(result))
}
