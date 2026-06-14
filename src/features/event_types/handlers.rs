use axum::{
    extract::{Extension, Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::{
    common::{
        idempotency,
        qs_query::QsQuery,
        types::{PaginatedResponse, RequestContext},
        validators,
        ValidatedJson,
    },
    error::AppError,
    features::event_types::{
        models::{
            CreateEventTypeRequest, CreateVersionRequest, EventTypeResponse,
            EventTypeSchemaResponse, ListQueryParams, UpdateEventTypeRequest,
        },
        repository::EventTypeRepository,
        service::EventTypeService,
    },
    state::AppState,
};

fn svc(state: AppState) -> EventTypeService {
    EventTypeService::new(EventTypeRepository::new(state.db))
}

pub async fn create_event_type(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<CreateEventTypeRequest>,
) -> Result<(StatusCode, Json<EventTypeResponse>), AppError> {
    if let Some(key) = idempotency::extract_key(&headers)? {
        let hash = idempotency::body_hash(&payload);
        let redis = state.redis.clone();
        let et = idempotency::resolve(
            &redis,
            "event_types",
            &key,
            &hash,
            move || async move { svc(state).create(payload, ctx).await },
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(et)));
    }

    let et = svc(state).create(payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(et)))
}

pub async fn create_version(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<CreateVersionRequest>,
) -> Result<(StatusCode, Json<EventTypeResponse>), AppError> {
    if let Some(key) = idempotency::extract_key(&headers)? {
        let hash = idempotency::body_hash(&payload);
        let redis = state.redis.clone();
        let ns = format!("event_type_versions:{}", public_id);
        let et = idempotency::resolve(
            &redis,
            &ns,
            &key,
            &hash,
            move || async move {
                svc(state).create_version(public_id, payload, ctx).await
            },
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(et)));
    }

    let et = svc(state).create_version(public_id, payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(et)))
}

pub async fn list_event_types(
    State(state): State<AppState>,
    QsQuery(params): QsQuery<ListQueryParams>,
) -> Result<(StatusCode, Json<PaginatedResponse<EventTypeResponse>>), AppError> {
    let result = svc(state).list(params).await?;
    Ok((StatusCode::OK, Json(result)))
}

pub async fn get_event_type(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<EventTypeResponse>), AppError> {
    validators::validate_id_prefix(&public_id, "evt_", "event type")?;
    let et = svc(state).get_by_id(public_id).await?;
    Ok((StatusCode::OK, Json(et)))
}

pub async fn get_versions(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<Vec<EventTypeResponse>>), AppError> {
    let versions = svc(state).get_versions(public_id).await?;
    Ok((StatusCode::OK, Json(versions)))
}

pub async fn get_schema(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<EventTypeSchemaResponse>), AppError> {
    let schema = svc(state).get_schema(public_id).await?;
    Ok((StatusCode::OK, Json(schema)))
}

pub async fn update_event_type(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateEventTypeRequest>,
) -> Result<(StatusCode, Json<EventTypeResponse>), AppError> {
    let et = svc(state).update_description(public_id, payload, ctx).await?;
    Ok((StatusCode::OK, Json(et)))
}

pub async fn delete_event_type(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    svc(state).delete_by_id(public_id, ctx).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn archive_event_type(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<EventTypeResponse>), AppError> {
    let et = svc(state).archive(public_id, ctx).await?;
    Ok((StatusCode::OK, Json(et)))
}

pub async fn unarchive_event_type(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<EventTypeResponse>), AppError> {
    let et = svc(state).unarchive(public_id, ctx).await?;
    Ok((StatusCode::OK, Json(et)))
}
