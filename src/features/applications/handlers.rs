use axum::{extract::{Path, State}, http::StatusCode, Json};

use crate::{
    common::{PublicUuid, types::{ApiResponse, RequestContext}},
    error::AppError,
    features::applications::{
        models::{CreateApplicationRequest, CreateApplicationResponse, GetApplicationResponse},
        repository::ApplicationRepository,
        service::ApplicationService,
    },
    state::AppState,
};

fn make_ctx() -> RequestContext {
    RequestContext {
        request_id: PublicUuid::new_v7().into_inner(),
        created_by: PublicUuid::new_v7().into_inner(),
    }
}

pub async fn create_application(
    State(state): State<AppState>,
    Json(payload): Json<CreateApplicationRequest>,
) -> Result<(StatusCode, Json<ApiResponse<CreateApplicationResponse>>), AppError> {
    let service = ApplicationService::new(ApplicationRepository::new(state.db));
    let application = service.create(payload, make_ctx()).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: CreateApplicationResponse::from(application),
        }),
    ))
}

pub async fn get_by_id(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<ApiResponse<GetApplicationResponse>>), AppError> {
    let service = ApplicationService::new(ApplicationRepository::new(state.db));
    let application = service.get_by_id(public_id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: application,
        }),
    ))
}

pub async fn delete_by_id(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let service = ApplicationService::new(ApplicationRepository::new(state.db));
    service.delete_by_id(public_id, make_ctx()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore_by_id(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<ApiResponse<GetApplicationResponse>>), AppError> {
    let service = ApplicationService::new(ApplicationRepository::new(state.db));
    let application = service.restore_by_id(public_id, make_ctx()).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            data: application,
        }),
    ))
}
