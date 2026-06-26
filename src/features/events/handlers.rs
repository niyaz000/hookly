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
        validators, ValidatedJson,
    },
    error::AppError,
    features::{
        delivery::repository::DeliveryRepository,
        events::{
            models::{
                BulkCreateEventRequest, BulkCreateResponse, CreateEventRequest, EventResponse,
                ListQueryParams,
            },
            repository::EventRepository,
            service::EventService,
        },
        subscriptions::repository::SubscriptionRepository,
    },
    state::AppState,
};

fn svc(state: AppState) -> EventService {
    EventService::new(
        EventRepository::new(state.db.clone()),
        DeliveryRepository::new(state.db.clone()),
        SubscriptionRepository::new(state.db.clone()),
        state.redis,
    )
}

pub async fn create_event(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<CreateEventRequest>,
) -> Result<(StatusCode, Json<EventResponse>), AppError> {
    let idempotency_key = idempotency::extract_key(&headers)?;
    let (ev, created) = svc(state)
        .create(payload, ctx, idempotency_key.as_deref())
        .await?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(ev)))
}

pub async fn create_events_bulk(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    ValidatedJson(payload): ValidatedJson<BulkCreateEventRequest>,
) -> Result<(StatusCode, Json<BulkCreateResponse>), AppError> {
    if payload.events.is_empty() || payload.events.len() > 10 {
        return Err(AppError::BadRequest(
            "Bulk request must contain between 1 and 10 events".into(),
        ));
    }
    let response = svc(state).create_bulk(payload, ctx).await;
    Ok((StatusCode::MULTI_STATUS, Json(response)))
}

pub async fn list_events(
    State(state): State<AppState>,
    QsQuery(params): QsQuery<ListQueryParams>,
) -> Result<(StatusCode, Json<PaginatedResponse<EventResponse>>), AppError> {
    let result = svc(state).list(params).await?;
    Ok((StatusCode::OK, Json(result)))
}

pub async fn get_event(
    State(state): State<AppState>,
    Path(evt_id): Path<String>,
) -> Result<(StatusCode, Json<EventResponse>), AppError> {
    validators::validate_id_prefix(&evt_id, "evn_", "event")?;
    let ev = svc(state).get_by_id(evt_id).await?;
    Ok((StatusCode::OK, Json(ev)))
}
