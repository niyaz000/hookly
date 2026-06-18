use sha2::{Digest, Sha256};
use serde::Serialize;

use crate::error::AppError;

const LOCK_TTL_MS: u64 = 60_000;

/// Extracts and validates the `Idempotency-Key` header.
/// Returns `None` if the header is absent (normal flow).
/// Returns `Err` if the header value is invalid (empty or > 64 chars).
pub fn extract_key(headers: &axum::http::HeaderMap) -> Result<Option<String>, AppError> {
    let Some(val) = headers.get("Idempotency-Key") else {
        return Ok(None);
    };
    let s = val
        .to_str()
        .map_err(|_| AppError::BadRequest("Idempotency-Key must be valid ASCII".into()))?;
    if s.is_empty() {
        return Err(AppError::BadRequest(
            "Idempotency-Key must not be empty".into(),
        ));
    }
    if s.len() > 64 {
        return Err(AppError::BadRequest(
            "Idempotency-Key must be 64 characters or fewer".into(),
        ));
    }
    Ok(Some(s.to_owned()))
}

/// Returns the SHA-256 of the canonical JSON as raw bytes for BYTEA storage.
/// Re-serializing normalizes whitespace and key ordering.
pub fn body_hash_bytes<T: Serialize>(body: &T) -> Vec<u8> {
    let bytes = serde_json::to_vec(body).unwrap_or_default();
    Sha256::digest(&bytes).to_vec()
}

/// Acquires the distributed lock. Returns the lock token on success.
/// Returns 409 if another request holds the lock.
pub async fn acquire_lock(
    redis: &redis::Client,
    namespace: &str,
    key: &str,
) -> Result<String, AppError> {
    let lock_key = format!("idmp_lock:{}:{}", namespace, key);
    let lock_token = uuid::Uuid::new_v4().to_string();

    let mut conn = redis
        .get_multiplexed_async_connection()
        .await
        .map_err(AppError::Redis)?;

    let acquired: Option<String> = redis::cmd("SET")
        .arg(&lock_key)
        .arg(&lock_token)
        .arg("NX")
        .arg("PX")
        .arg(LOCK_TTL_MS)
        .query_async(&mut conn)
        .await
        .map_err(AppError::Redis)?;

    if acquired.is_none() {
        return Err(AppError::Conflict(
            "A concurrent request with this idempotency key is already in progress".into(),
            vec![],
        ));
    }

    Ok(lock_token)
}

/// Releases the lock using an ABA-safe Lua DEL — only deletes if the stored token still
/// matches. A best-effort release: failures are swallowed (lock expires after 60 s anyway).
pub async fn release_lock(redis: &redis::Client, namespace: &str, key: &str, token: &str) {
    let lock_key = format!("idmp_lock:{}:{}", namespace, key);
    let script = r#"
        if redis.call("get", KEYS[1]) == ARGV[1] then
            return redis.call("del", KEYS[1])
        else
            return 0
        end
    "#;
    if let Ok(mut conn) = redis.get_multiplexed_async_connection().await {
        let _: i64 = redis::Script::new(script)
            .key(&lock_key)
            .arg(token)
            .invoke_async(&mut conn)
            .await
            .unwrap_or(0);
    }
}
