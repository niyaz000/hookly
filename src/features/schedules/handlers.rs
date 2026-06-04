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
    features::schedules::{
        models::{
            CreateScheduleRequest, ListExecutionsQuery, ListExecutionsResponse, ListSchedulesQuery,
            ListSchedulesResponse, ScheduleExecutionResponse, ScheduleResponse,
            UpdateScheduleRequest,
        },
        repository::ScheduleRepository,
        service::ScheduleService,
    },
    state::AppState,
};

fn service(state: AppState) -> ScheduleService {
    ScheduleService::new(ScheduleRepository::new(state.db))
}

pub async fn create_schedule(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<CreateScheduleRequest>,
) -> Result<(StatusCode, Json<ScheduleResponse>), AppError> {
    if let Some(key) = idempotency::extract_key(&headers)? {
        let hash = idempotency::body_hash(&payload);
        let redis = state.redis.clone();
        let schedule = idempotency::resolve(
            &redis,
            "schedules",
            &key,
            &hash,
            move || async move { service(state).create(payload, ctx).await },
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(schedule)));
    }

    let schedule = service(state).create(payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(schedule)))
}

pub async fn get_schedule(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<ScheduleResponse>, AppError> {
    let schedule = service(state).get_by_public_id(public_id).await?;
    Ok(Json(schedule))
}

pub async fn update_schedule(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateScheduleRequest>,
) -> Result<Json<ScheduleResponse>, AppError> {
    let schedule = service(state).update(public_id, payload, ctx).await?;
    Ok(Json(schedule))
}

pub async fn delete_schedule(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    service(state).delete(public_id, ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore_schedule(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<ScheduleResponse>, AppError> {
    let schedule = service(state).restore(public_id, ctx).await?;
    Ok(Json(schedule))
}

pub async fn pause_schedule(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<ScheduleResponse>, AppError> {
    let schedule = service(state).pause(public_id, ctx).await?;
    Ok(Json(schedule))
}

pub async fn resume_schedule(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<Json<ScheduleResponse>, AppError> {
    let schedule = service(state).resume(public_id, ctx).await?;
    Ok(Json(schedule))
}

pub async fn trigger_schedule(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<ScheduleExecutionResponse>), AppError> {
    let execution = service(state).trigger(public_id, ctx).await?;
    Ok((StatusCode::ACCEPTED, Json(execution)))
}

pub async fn list_schedules(
    State(state): State<AppState>,
    QsQuery(query): QsQuery<ListSchedulesQuery>,
) -> Result<Json<ListSchedulesResponse>, AppError> {
    let result = service(state).list(query).await?;
    Ok(Json(result))
}

pub async fn list_executions(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
    QsQuery(query): QsQuery<ListExecutionsQuery>,
) -> Result<Json<ListExecutionsResponse>, AppError> {
    let result = service(state).list_executions(public_id, query).await?;
    Ok(Json(result))
}

pub async fn get_execution(
    State(state): State<AppState>,
    Path((public_id, exec_public_id)): Path<(String, String)>,
) -> Result<Json<ScheduleExecutionResponse>, AppError> {
    let execution = service(state).get_execution(public_id, exec_public_id).await?;
    Ok(Json(execution))
}
