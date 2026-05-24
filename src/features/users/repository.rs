use sqlx::PgPool;
use crate::features::users::models::{User, CreateUserRequest};
use crate::error::AppError;
use uuid::Uuid;

pub struct UserRepository {
    pool: PgPool,
}

impl UserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, req: CreateUserRequest) -> Result<User, AppError> {
        let user = sqlx::query_as::<_, User>(
            r#"INSERT INTO users (name, email) VALUES ($1, $2) RETURNING *"#,
        )
        .bind(req.name)
        .bind(req.email)
        .fetch_one(&self.pool)
        .await?;

        Ok(user)
    }

    pub async fn get_by_id(&self, id: Uuid) -> Result<Option<User>, AppError> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }
}
