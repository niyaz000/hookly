use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    Json,
};

use crate::{
    common::{
        qs_query::QsQuery,
        types::{PaginatedResponse, RequestContext},
        validators, ValidatedJson,
    },
    error::AppError,
    state::AppState,
};

use super::{
    models::{CreateSubscriptionRequest, ListQueryParams, SubscriptionResponse},
    repository::SubscriptionRepository,
    service::SubscriptionService,
};

fn svc(state: AppState) -> SubscriptionService {
    SubscriptionService::new(SubscriptionRepository::new(state.db))
}

pub async fn create_subscription(
    State(state): State<AppState>,
    Extension(ctx): Extension<RequestContext>,
    ValidatedJson(payload): ValidatedJson<CreateSubscriptionRequest>,
) -> Result<(StatusCode, Json<SubscriptionResponse>), AppError> {
    let sub = svc(state).create(payload, ctx).await?;
    Ok((StatusCode::CREATED, Json(sub)))
}

pub async fn get_subscription(
    State(state): State<AppState>,
    Path(sub_id): Path<String>,
) -> Result<(StatusCode, Json<SubscriptionResponse>), AppError> {
    validators::validate_id_prefix(&sub_id, "sub_", "subscription")?;
    let sub = svc(state).get_by_id(&sub_id).await?;
    Ok((StatusCode::OK, Json(sub)))
}

pub async fn list_subscriptions(
    State(state): State<AppState>,
    QsQuery(params): QsQuery<ListQueryParams>,
) -> Result<(StatusCode, Json<PaginatedResponse<SubscriptionResponse>>), AppError> {
    let result = svc(state).list(params).await?;
    Ok((StatusCode::OK, Json(result)))
}

pub async fn delete_subscription(
    State(state): State<AppState>,
    Path(sub_id): Path<String>,
) -> Result<StatusCode, AppError> {
    validators::validate_id_prefix(&sub_id, "sub_", "subscription")?;
    svc(state).delete(&sub_id).await?;
    Ok(StatusCode::NO_CONTENT)
}
