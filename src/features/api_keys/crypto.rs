use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};

use crate::common::NanoId;
use crate::error::AppError;

/// Generates a new API key and its 3-char display prefix.
/// Returns `(full_key, key_prefix)`.
///
/// Format: `hkly_<env>_<base62_random>`
pub fn generate_api_key(env_str: &str, key_length: i16) -> (String, String) {
    let random_part = NanoId::generate(key_length as usize);
    let key_prefix: String = random_part.chars().take(3).collect();
    let full_key = format!("hkly_{}_{}", env_str, random_part);
    (full_key, key_prefix)
}

/// SHA-256 hash of the key encoded as base64url — used as the auth lookup token.
pub fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

/// AES-256-GCM encrypt `plaintext` with the given 32-byte key.
/// Returns a versioned envelope: `v1$<nonce_b64url>$<ciphertext_b64url>`.
pub fn encrypt_key(raw_key: [u8; 32], plaintext: &str) -> Result<String, AppError> {
    let key = Key::<Aes256Gcm>::from_slice(&raw_key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| AppError::Internal("api key encryption failed".into()))?;

    Ok(format!(
        "v1${}${}",
        URL_SAFE_NO_PAD.encode(nonce),
        URL_SAFE_NO_PAD.encode(ciphertext)
    ))
}

/// AES-256-GCM decrypt an envelope produced by `encrypt_key`.
pub fn decrypt_key(raw_key: [u8; 32], envelope: &str) -> Result<String, AppError> {
    let parts: Vec<&str> = envelope.splitn(3, '$').collect();
    if parts.len() != 3 || parts[0] != "v1" {
        return Err(AppError::Internal("invalid api key envelope format".into()));
    }

    let nonce_bytes = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| AppError::Internal("invalid nonce encoding in api key envelope".into()))?;
    let ciphertext = URL_SAFE_NO_PAD
        .decode(parts[2])
        .map_err(|_| AppError::Internal("invalid ciphertext encoding in api key envelope".into()))?;

    let key = Key::<Aes256Gcm>::from_slice(&raw_key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .map_err(|_| AppError::Internal("api key decryption failed".into()))?;

    String::from_utf8(plaintext)
        .map_err(|_| AppError::Internal("decrypted api key contains invalid UTF-8".into()))
}
