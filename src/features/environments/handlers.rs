use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{qs_query::QsQuery, types::RequestContext, ValidatedJson},
    error::AppError,
    state::AppState,
};

use super::{
    models::{
        CreateEnvironmentRequest, EnvironmentResponse, ListEnvironmentsQuery,
        ListEnvironmentsResponse, UpdateEnvironmentRequest,
    },
    repository::EnvironmentRepository,
    service::EnvironmentService,
};

fn make_svc(state: AppState) -> EnvironmentService {
    EnvironmentService::new(EnvironmentRepository::new(state.db))
}

pub async fn create_environment(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    ValidatedJson(payload): ValidatedJson<CreateEnvironmentRequest>,
) -> Result<(StatusCode, Json<EnvironmentResponse>), AppError> {
    let payload = payload.normalize();
    payload.validate_all()?;

    let svc = make_svc(state);
    let env = svc.create(payload, ctx).await?;

    Ok((StatusCode::CREATED, Json(EnvironmentResponse::from(env))))
}

pub async fn get_environment(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<EnvironmentResponse>), AppError> {
    let svc = make_svc(state);
    let env = svc.get_by_id(&public_id).await?;

    Ok((StatusCode::OK, Json(EnvironmentResponse::from(env))))
}

pub async fn list_environments(
    State(state): State<AppState>,
    QsQuery(query): QsQuery<ListEnvironmentsQuery>,
) -> Result<(StatusCode, Json<ListEnvironmentsResponse>), AppError> {
    let tenant_id = query
        .tenant_id
        .ok_or_else(|| AppError::BadRequest("tenant_id query parameter is required".into()))?;

    let svc = make_svc(state);
    let resp = svc.list(tenant_id, query).await?;

    Ok((StatusCode::OK, Json(resp)))
}

pub async fn update_environment(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateEnvironmentRequest>,
) -> Result<(StatusCode, Json<EnvironmentResponse>), AppError> {
    let svc = make_svc(state);
    let env = svc.update(&public_id, payload, ctx).await?;

    Ok((StatusCode::OK, Json(EnvironmentResponse::from(env))))
}

pub async fn enable_environment(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<EnvironmentResponse>), AppError> {
    let svc = make_svc(state);
    let env = svc.enable(&public_id, ctx).await?;

    Ok((StatusCode::OK, Json(EnvironmentResponse::from(env))))
}

pub async fn disable_environment(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<EnvironmentResponse>), AppError> {
    let svc = make_svc(state);
    let env = svc.disable(&public_id, ctx).await?;

    Ok((StatusCode::OK, Json(EnvironmentResponse::from(env))))
}
