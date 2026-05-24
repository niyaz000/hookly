use crate::features::users::{models::{User, CreateUserRequest}, repository::UserRepository};
use crate::error::AppError;

pub struct UserService {
    repo: UserRepository,
}

impl UserService {
    pub fn new(repo: UserRepository) -> Self {
        Self { repo }
    }

    pub async fn create_user(&self, req: CreateUserRequest) -> Result<User, AppError> {
        self.repo.create(req).await
    }
}
