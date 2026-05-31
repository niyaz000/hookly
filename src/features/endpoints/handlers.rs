use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{
        types::{PaginatedResponse, RequestContext},
        PublicUuid, ValidatedJson,
    },
    error::AppError,
    features::endpoints::{
        models::{
            CreateEndpointRequest, EndpointResponse, ListQueryParams, RotateSecretRequest,
            SecretResponse, UpdateEndpointRequest,
        },
        repository::EndpointRepository,
        service::EndpointService,
    },
    state::AppState,
};

fn make_ctx() -> RequestContext {
    RequestContext {
        request_id: PublicUuid::new_v7().into_inner(),
        created_by: PublicUuid::new_v7().into_inner(),
    }
}

fn svc(state: AppState) -> EndpointService {
    EndpointService::new(EndpointRepository::new(state.db), state.crypto)
}

pub async fn create_endpoint(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<CreateEndpointRequest>,
) -> Result<(StatusCode, Json<EndpointResponse>), AppError> {
    let ep = svc(state).create(payload, make_ctx()).await?;
    Ok((StatusCode::CREATED, Json(ep)))
}

pub async fn list_endpoints(
    State(state): State<AppState>,
    Query(params): Query<ListQueryParams>,
) -> Result<(StatusCode, Json<PaginatedResponse<EndpointResponse>>), AppError> {
    let result = svc(state).list(params).await?;
    Ok((StatusCode::OK, Json(result)))
}

pub async fn get_endpoint(
    State(state): State<AppState>,
    Path(ep_id): Path<String>,
) -> Result<(StatusCode, Json<EndpointResponse>), AppError> {
    let ep = svc(state).get_by_id(ep_id).await?;
    Ok((StatusCode::OK, Json(ep)))
}

pub async fn update_endpoint(
    State(state): State<AppState>,
    Path(ep_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateEndpointRequest>,
) -> Result<(StatusCode, Json<EndpointResponse>), AppError> {
    let ep = svc(state).update(ep_id, payload, make_ctx()).await?;
    Ok((StatusCode::OK, Json(ep)))
}

pub async fn delete_endpoint(
    State(state): State<AppState>,
    Path(ep_id): Path<String>,
) -> Result<StatusCode, AppError> {
    svc(state).delete(ep_id, make_ctx()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn pause_endpoint(
    State(state): State<AppState>,
    Path(ep_id): Path<String>,
) -> Result<(StatusCode, Json<EndpointResponse>), AppError> {
    let ep = svc(state).pause(ep_id, make_ctx()).await?;
    Ok((StatusCode::OK, Json(ep)))
}

pub async fn resume_endpoint(
    State(state): State<AppState>,
    Path(ep_id): Path<String>,
) -> Result<(StatusCode, Json<EndpointResponse>), AppError> {
    let ep = svc(state).resume(ep_id, make_ctx()).await?;
    Ok((StatusCode::OK, Json(ep)))
}

pub async fn get_secret(
    State(state): State<AppState>,
    Path(ep_id): Path<String>,
) -> Result<(StatusCode, Json<SecretResponse>), AppError> {
    let secret = svc(state).get_secret(ep_id).await?;
    Ok((StatusCode::OK, Json(secret)))
}

pub async fn rotate_secret(
    State(state): State<AppState>,
    Path(ep_id): Path<String>,
    payload: Option<Json<RotateSecretRequest>>,
) -> Result<(StatusCode, Json<SecretResponse>), AppError> {
    let req = payload.map(|Json(r)| r).unwrap_or_default();
    let secret = svc(state).rotate_secret(ep_id, req, make_ctx()).await?;
    Ok((StatusCode::OK, Json(secret)))
}
