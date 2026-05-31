use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};

use crate::{
    common::{
        idempotency,
        types::RequestContext,
        PublicUuid, ValidatedJson,
    },
    error::AppError,
    features::users::{
        models::{
            CreateUserRequest, ListUsersQuery, ListUsersResponse, LockUserRequest,
            UpdateUserRequest, UserResponse,
        },
        repository::UserRepository,
        service::UserService,
    },
    state::AppState,
};

fn make_ctx() -> RequestContext {
    RequestContext {
        request_id: PublicUuid::new_v7().into_inner(),
        created_by: PublicUuid::new_v7().into_inner(),
    }
}

fn service(state: AppState) -> UserService {
    UserService::new(UserRepository::new(state.db))
}

pub async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    ValidatedJson(payload): ValidatedJson<CreateUserRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    if let Some(key) = idempotency::extract_key(&headers)? {
        let hash = idempotency::body_hash(&payload);
        let redis = state.redis.clone();
        let user = idempotency::resolve(
            &redis,
            "users",
            &key,
            &hash,
            move || async move { service(state).create(payload, make_ctx()).await },
        )
        .await?;
        return Ok((StatusCode::CREATED, Json(user)));
    }

    let user = service(state).create(payload, make_ctx()).await?;
    Ok((StatusCode::CREATED, Json(user)))
}

pub async fn get_user(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<UserResponse>, AppError> {
    let user = service(state).get_by_public_id(public_id).await?;
    Ok(Json(user))
}

pub async fn update_user(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<UpdateUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let user = service(state).update(public_id, payload, make_ctx()).await?;
    Ok(Json(user))
}

pub async fn delete_user(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<StatusCode, AppError> {
    service(state).delete(public_id, make_ctx()).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn suspend_user(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<UserResponse>, AppError> {
    let user = service(state).suspend(public_id, make_ctx()).await?;
    Ok(Json(user))
}

pub async fn reactivate_user(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<UserResponse>, AppError> {
    let user = service(state).reactivate(public_id, make_ctx()).await?;
    Ok(Json(user))
}

pub async fn lock_user(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
    ValidatedJson(payload): ValidatedJson<LockUserRequest>,
) -> Result<Json<UserResponse>, AppError> {
    let user = service(state).lock(public_id, payload, make_ctx()).await?;
    Ok(Json(user))
}

pub async fn unlock_user(
    State(state): State<AppState>,
    Path(public_id): Path<String>,
) -> Result<Json<UserResponse>, AppError> {
    let user = service(state).unlock(public_id, make_ctx()).await?;
    Ok(Json(user))
}

pub async fn list_users(
    State(state): State<AppState>,
    Query(query): Query<ListUsersQuery>,
) -> Result<Json<ListUsersResponse>, AppError> {
    let result = service(state).list(query).await?;
    Ok(Json(result))
}
