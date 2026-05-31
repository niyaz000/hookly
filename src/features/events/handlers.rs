use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::{
    common::{
        idempotency,
        types::{PaginatedResponse, RequestContext},
        PublicUuid, ValidatedJson,
    },
    error::AppError,
    features::{
        delivery::repository::DeliveryRepository,
        events::{
            models::{CreateEventRequest, EventResponse, ListQueryParams},
            repository::EventRepository,
            service::EventService,
        },
    },
    state::AppState,
};

fn make_ctx() -> RequestContext {
    RequestContext {
        request_id: PublicUuid::new_v7().into_inner(),
        created_by: PublicUuid::new_v7().into_inner(),
    }
}

fn svc(state: AppState) -> EventService {
    EventService::new(
        EventRepository::new(state.db.clone()),
        DeliveryRepository::new(state.db),
        state.redis,
    )
}

pub async fn create_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<CreateEventRequest>,
) -> Result<(StatusCode, Json<EventResponse>), AppError> {
    if let Some(key) = idempotency::extract_key(&headers)? {
        let hash = idempotency::body_hash(&payload);
        let redis = state.redis.clone();
        let ev = idempotency::resolve(
            &redis,
            "events",
            &key,
            &hash,
            move || async move {
                let (ev, _) = svc(state).create(payload, make_ctx()).await?;
                Ok(ev)
            },
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(ev)));
    }

    let (ev, created) = svc(state).create(payload, make_ctx()).await?;
    let status = if created { StatusCode::CREATED } else { StatusCode::OK };
    Ok((status, Json(ev)))
}

pub async fn list_events(
    State(state): State<AppState>,
    Query(params): Query<ListQueryParams>,
) -> Result<(StatusCode, Json<PaginatedResponse<EventResponse>>), AppError> {
    let result = svc(state).list(params).await?;
    Ok((StatusCode::OK, Json(result)))
}

pub async fn get_event(
    State(state): State<AppState>,
    Path(evt_id): Path<String>,
) -> Result<(StatusCode, Json<EventResponse>), AppError> {
    let ev = svc(state).get_by_id(evt_id).await?;
    Ok((StatusCode::OK, Json(ev)))
}
