use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::ValidatedJson,
    error::AppError,
    features::platform_event_types::repository::PlatformEventTypeRepository,
    state::AppState,
};

use super::{
    models::{
        ListSubscriptionsQuery, ListSubscriptionsResponse, ReplaceSubscriptionsRequest,
        ReplaceSubscriptionsResponse, SubscribeRequest, SubscribeResponse,
        SubscriptionItemResponse, UnsubscribeQuery,
    },
    repository::PlatformSubscriptionRepository,
};

fn sub_repo(state: &AppState) -> PlatformSubscriptionRepository {
    PlatformSubscriptionRepository::new(state.db.clone())
}

fn et_repo(state: &AppState) -> PlatformEventTypeRepository {
    PlatformEventTypeRepository::new(state.db.clone())
}

/// Validates the given event_type_ids against the platform_event_types table.
/// Returns (valid_ids, invalid_ids).
async fn partition_valid_ids(
    et_repo: &PlatformEventTypeRepository,
    ids: &[String],
) -> Result<(Vec<String>, Vec<String>), AppError> {
    let found = et_repo.get_public_ids_by_ids(ids).await?;
    let found_set: std::collections::HashSet<&str> = found.iter().map(|s| s.as_str()).collect();
    let valid: Vec<String> = ids.iter().filter(|id| found_set.contains(id.as_str())).cloned().collect();
    let invalid: Vec<String> = ids.iter().filter(|id| !found_set.contains(id.as_str())).cloned().collect();
    Ok((valid, invalid))
}

pub async fn list_subscriptions(
    State(state): State<AppState>,
    Query(query): Query<ListSubscriptionsQuery>,
) -> Result<(StatusCode, Json<ListSubscriptionsResponse>), AppError> {
    let subs = sub_repo(&state).list_for_tenant(query.tenant_id).await?;
    Ok((
        StatusCode::OK,
        Json(ListSubscriptionsResponse {
            tenant_id: query.tenant_id,
            items: subs.into_iter().map(SubscriptionItemResponse::from).collect(),
        }),
    ))
}

pub async fn subscribe(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<SubscribeRequest>,
) -> Result<(StatusCode, Json<SubscribeResponse>), AppError> {
    payload.validate_all()?;

    let (valid_ids, invalid_ids) =
        partition_valid_ids(&et_repo(&state), &payload.event_type_ids).await?;

    let (inserted, already_present) =
        sub_repo(&state).subscribe(payload.tenant_id, &valid_ids).await?;

    Ok((
        StatusCode::OK,
        Json(SubscribeResponse {
            tenant_id: payload.tenant_id,
            subscribed: inserted,
            already_present,
            invalid_event_type_ids: invalid_ids,
        }),
    ))
}

pub async fn replace_subscriptions(
    State(state): State<AppState>,
    ValidatedJson(payload): ValidatedJson<ReplaceSubscriptionsRequest>,
) -> Result<(StatusCode, Json<ReplaceSubscriptionsResponse>), AppError> {
    payload.validate_all()?;

    let (valid_ids, invalid_ids) =
        partition_valid_ids(&et_repo(&state), &payload.event_type_ids).await?;

    let (inserted, removed) =
        sub_repo(&state).replace(payload.tenant_id, &valid_ids).await?;

    Ok((
        StatusCode::OK,
        Json(ReplaceSubscriptionsResponse {
            tenant_id: payload.tenant_id,
            subscribed: inserted,
            removed,
            invalid_event_type_ids: invalid_ids,
        }),
    ))
}

pub async fn unsubscribe(
    State(state): State<AppState>,
    Query(query): Query<UnsubscribeQuery>,
) -> Result<StatusCode, AppError> {
    let deleted = sub_repo(&state)
        .unsubscribe(query.tenant_id, &query.event_type_id)
        .await?;
    if !deleted {
        return Err(AppError::NotFound(format!(
            "subscription not found for event_type_id: {}",
            query.event_type_id
        )));
    }
    Ok(StatusCode::NO_CONTENT)
}

