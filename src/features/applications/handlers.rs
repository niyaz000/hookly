use axum::{
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::{
    common::{
        idempotency,
        types::RequestContext,
        ValidatedJson,
    },
    error::AppError,
    features::applications::{
        models::{CreateApplicationRequest, CreateApplicationResponse, GetApplicationResponse},
        repository::ApplicationRepository,
        service::ApplicationService,
    },
    state::AppState,
};

pub async fn create_application(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<CreateApplicationRequest>,
) -> Result<(StatusCode, Json<CreateApplicationResponse>), AppError> {
    if let Some(key) = idempotency::extract_key(&headers)? {
        let hash = idempotency::body_hash(&payload);
        let redis = state.redis.clone();
        let resp = idempotency::resolve(
            &redis,
            "applications",
            &key,
            &hash,
            move || async move {
                let svc = ApplicationService::new(ApplicationRepository::new(state.db));
                let app = svc.create(payload, ctx).await?;
                Ok(CreateApplicationResponse::from(app))
            },
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(resp)));
    }

    let svc = ApplicationService::new(ApplicationRepository::new(state.db));
    let app = svc.create(payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(CreateApplicationResponse::from(app))))
}

pub async fn get_by_id(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<GetApplicationResponse>), AppError> {
    let service = ApplicationService::new(ApplicationRepository::new(state.db));
    let application = service.get_by_id(public_id).await?;
    Ok((StatusCode::OK, Json(application)))
}

pub async fn delete_by_id(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let service = ApplicationService::new(ApplicationRepository::new(state.db));
    service.delete_by_id(public_id, ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore_by_id(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<GetApplicationResponse>), AppError> {
    let service = ApplicationService::new(ApplicationRepository::new(state.db));
    let application = service.restore_by_id(public_id, ctx).await?;
    Ok((StatusCode::OK, Json(application)))
}
