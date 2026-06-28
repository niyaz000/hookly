use std::sync::Arc;

use redis::Client as RedisClient;
use sqlx::PgPool;

use crate::common::{EnvKeyProvider, KeyProvider, TenantCrypto};
use crate::config::Config;
use crate::email::{EmailService, NoopEmailService};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    #[allow(dead_code)]
    pub redis: RedisClient,
    pub crypto: TenantCrypto,
    pub email: Arc<dyn EmailService>,
    pub key_provider: Arc<dyn KeyProvider>,
    pub admin_api_key: String,
}

impl AppState {
    pub async fn new(config: &Config) -> Result<Self, sqlx::Error> {
        let db = PgPool::connect(&config.database.url).await?;
        let redis = RedisClient::open(config.redis.url.as_str()).expect("Invalid Redis URL");
        let crypto =
            TenantCrypto::new(&config.crypto.master_key).expect("Invalid CRYPTO_MASTER_KEY");
        let email: Arc<dyn EmailService> = Arc::new(NoopEmailService);
        let key_provider: Arc<dyn KeyProvider> = Arc::new(
            EnvKeyProvider::from_b64(&config.crypto.api_key_encryption_key)
                .expect("Invalid CRYPTO_API_KEY_ENCRYPTION_KEY"),
        );

        Ok(Self {
            db,
            redis,
            crypto,
            email,
            key_provider,
            admin_api_key: config.admin.api_key.clone(),
        })
    }
}
