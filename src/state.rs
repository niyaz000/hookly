use sqlx::PgPool;
use redis::Client as RedisClient;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub redis: RedisClient,
}

impl AppState {
    pub async fn new(config: &Config) -> Result<Self, sqlx::Error> {
        let db = PgPool::connect(&config.database.url).await?;
        
        let redis = RedisClient::open(config.redis.url.as_str())
            .expect("Invalid Redis URL");

        Ok(Self { db, redis })
    }
}
