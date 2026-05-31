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
    ValidatedJson(payload): ValidatedJson<CreateEventRequest>,
) -> Result<(StatusCode, Json<EventResponse>), AppError> {
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
