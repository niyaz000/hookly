use std::sync::Arc;

use redis::Client as RedisClient;
use sqlx::PgPool;

use crate::common::TenantCrypto;
use crate::config::Config;
use crate::email::{EmailService, NoopEmailService};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    #[allow(dead_code)]
    pub redis: RedisClient,
    pub crypto: TenantCrypto,
    pub email: Arc<dyn EmailService>,
}

impl AppState {
    pub async fn new(config: &Config) -> Result<Self, sqlx::Error> {
        let db = PgPool::connect(&config.database.url).await?;
        let redis = RedisClient::open(config.redis.url.as_str()).expect("Invalid Redis URL");
        let crypto =
            TenantCrypto::new(&config.crypto.master_key).expect("Invalid CRYPTO_MASTER_KEY");
        let email: Arc<dyn EmailService> = Arc::new(NoopEmailService);

        Ok(Self { db, redis, crypto, email })
    }
}
