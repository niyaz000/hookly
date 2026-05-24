use axum::{extract::State, http::StatusCode, Json};

use crate::{
    common::{PublicUuid, types::{ApiResponse, RequestContext}},
    error::AppError,
    features::applications::{
        models::{CreateApplicationRequest, CreateApplicationResponse},
        repository::ApplicationRepository,
        service::ApplicationService,
    },
    state::AppState,
};

pub async fn create_application(
    State(state): State<AppState>,
    Json(payload): Json<CreateApplicationRequest>,
) -> Result<(StatusCode, Json<ApiResponse<CreateApplicationResponse>>), AppError> {
    let ctx = RequestContext {
        request_id: PublicUuid::new_v7().into_inner(),
        created_by: PublicUuid::new_v7().into_inner(),
    };

    let service = ApplicationService::new(ApplicationRepository::new(state.db));
    let application = service.create(payload, ctx).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse {
            success: true,
            data: CreateApplicationResponse::from(application),
        }),
    ))
}
