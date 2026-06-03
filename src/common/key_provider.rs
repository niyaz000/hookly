use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};

use crate::error::AppError;

#[async_trait]
pub trait KeyProvider: Send + Sync {
    async fn get_encryption_key(&self) -> Result<[u8; 32], AppError>;
}

pub struct EnvKeyProvider {
    key: [u8; 32],
}

impl EnvKeyProvider {
    /// `b64_key` must be standard base64 encoding of exactly 32 bytes.
    /// Generate with: `openssl rand -base64 32`
    pub fn from_b64(b64_key: &str) -> Result<Self, String> {
        let bytes = STANDARD
            .decode(b64_key.trim())
            .map_err(|_| "API_KEY_ENCRYPTION_KEY must be standard base64-encoded".to_string())?;
        if bytes.len() != 32 {
            return Err(format!(
                "API_KEY_ENCRYPTION_KEY must decode to 32 bytes, got {}",
                bytes.len()
            ));
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes);
        Ok(Self { key })
    }
}

#[async_trait]
impl KeyProvider for EnvKeyProvider {
    async fn get_encryption_key(&self) -> Result<[u8; 32], AppError> {
        Ok(self.key)
    }
}
