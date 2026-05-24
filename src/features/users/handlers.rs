use axum::{extract::State, Json};

use crate::{
    state::AppState,
    error::AppError,
    common::types::ApiResponse,
    features::users::{models::{CreateUserRequest, User}, service::UserService},
};

pub async fn create_user(
    State(state): State<AppState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<ApiResponse<User>>, AppError> {
    let service = UserService::new(crate::features::users::repository::UserRepository::new(state.db));
    let user = service.create_user(payload).await?;
    
    Ok(Json(ApiResponse { success: true, data: user }))
}
