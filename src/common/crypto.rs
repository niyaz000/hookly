use aes_gcm::{
    aead::{rand_core::RngCore, Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

use crate::error::AppError;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
pub struct TenantCrypto {
    master_key: Vec<u8>,
}

impl TenantCrypto {
    /// `master_key_b64` must be standard base64 encoding of exactly 32 bytes.
    /// Generate with: `openssl rand -base64 32`
    pub fn new(master_key_b64: &str) -> Result<Self, String> {
        let key = STANDARD
            .decode(master_key_b64.trim())
            .map_err(|_| "CRYPTO_MASTER_KEY must be standard base64-encoded".to_string())?;
        if key.len() != 32 {
            return Err(format!(
                "CRYPTO_MASTER_KEY must decode to 32 bytes, got {}",
                key.len()
            ));
        }
        Ok(Self { master_key: key })
    }

    /// Derives a 32-byte per-tenant key via HMAC-SHA256(master_key, tenant_id).
    fn derive_key(&self, tenant_id: Uuid) -> [u8; 32] {
        let mut mac = <HmacSha256 as Mac>::new_from_slice(&self.master_key)
            .expect("HMAC accepts any key size");
        mac.update(tenant_id.as_bytes());
        mac.finalize().into_bytes().into()
    }

    /// Encrypts `plaintext` for the given tenant.
    /// Returns a versioned envelope: `v1$<nonce_b64url>$<ciphertext_b64url>`.
    pub fn encrypt(&self, tenant_id: Uuid, plaintext: &str) -> Result<String, AppError> {
        let key_bytes = self.derive_key(tenant_id);
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| AppError::Internal("encryption failed".into()))?;

        Ok(format!(
            "v1${}${}",
            URL_SAFE_NO_PAD.encode(nonce),
            URL_SAFE_NO_PAD.encode(ciphertext)
        ))
    }

    /// Decrypts an envelope produced by `encrypt`.
    pub fn decrypt(&self, tenant_id: Uuid, encrypted: &str) -> Result<String, AppError> {
        let parts: Vec<&str> = encrypted.splitn(3, '$').collect();
        if parts.len() != 3 || parts[0] != "v1" {
            return Err(AppError::Internal("invalid encrypted secret format".into()));
        }

        let nonce_bytes = URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|_| AppError::Internal("invalid nonce encoding".into()))?;
        let ciphertext = URL_SAFE_NO_PAD
            .decode(parts[2])
            .map_err(|_| AppError::Internal("invalid ciphertext encoding".into()))?;

        let key_bytes = self.derive_key(tenant_id);
        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| AppError::Internal("decryption failed".into()))?;

        String::from_utf8(plaintext)
            .map_err(|_| AppError::Internal("invalid UTF-8 in decrypted secret".into()))
    }

    /// Generates a new `whsec_<base64url(32 random bytes)>` webhook signing secret.
    pub fn generate_webhook_secret() -> String {
        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);
        format!("whsec_{}", URL_SAFE_NO_PAD.encode(bytes))
    }
}
