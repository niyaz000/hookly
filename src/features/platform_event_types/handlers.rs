use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};

use crate::{error::AppError, state::AppState};

use super::{
    models::{ListPlatformEventTypesQuery, ListPlatformEventTypesResponse, PlatformEventTypeResponse},
    repository::PlatformEventTypeRepository,
};

fn repo(state: AppState) -> PlatformEventTypeRepository {
    PlatformEventTypeRepository::new(state.db)
}

pub async fn list_platform_event_types(
    State(state): State<AppState>,
    Query(query): Query<ListPlatformEventTypesQuery>,
) -> Result<(StatusCode, Json<ListPlatformEventTypesResponse>), AppError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let (ets, next_cursor) = repo(state).list(query.resource, limit, query.cursor).await?;
    Ok((
        StatusCode::OK,
        Json(ListPlatformEventTypesResponse {
            items: ets.into_iter().map(PlatformEventTypeResponse::from).collect(),
            next_cursor,
            limit,
        }),
    ))
}

pub async fn get_platform_event_type(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<(StatusCode, Json<PlatformEventTypeResponse>), AppError> {
    let et = repo(state)
        .get_by_public_id(&public_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("platform event type not found: {public_id}")))?;
    Ok((StatusCode::OK, Json(PlatformEventTypeResponse::from(et))))
}
