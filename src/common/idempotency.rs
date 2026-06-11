use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

use crate::error::AppError;

const RECORD_TTL_SECS: u64 = 86_400; // 24 hours
const LOCK_TTL_MS: u64 = 60_000; // 60 seconds

#[derive(Serialize, serde::Deserialize)]
struct StoredRecord {
    body_hash: String,
    response: serde_json::Value,
}

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

/// Computes a SHA-256 hash of the canonical JSON representation of `body`.
/// Re-serializing normalizes whitespace and key ordering.
pub fn body_hash<T: Serialize>(body: &T) -> String {
    let bytes = serde_json::to_vec(body).unwrap_or_default();
    format!("{:x}", Sha256::digest(&bytes))
}

/// Checks the idempotency record, executes `f` if this is a fresh request,
/// stores the result, and returns either the fresh or cached response.
///
/// Flow:
///   1. GET record — if found and hash matches, return cached response.
///   2. GET record — if found and hash differs, return 409.
///   3. Acquire distributed lock (SET NX). On failure return 409.
///   4. Execute `f`. On error, no record stored (request is retryable).
///   5. Store completed record, release lock (via Lua to avoid ABA race).
pub async fn resolve<T, F, Fut>(
    redis: &redis::Client,
    namespace: &str,
    key: &str,
    hash: &str,
    f: F,
) -> Result<T, AppError>
where
    T: Serialize + DeserializeOwned,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, AppError>>,
{
    let record_key = format!("idmp:{}:{}", namespace, key);
    let lock_key = format!("idmp_lock:{}:{}", namespace, key);

    let mut conn = redis
        .get_multiplexed_async_connection()
        .await
        .map_err(AppError::Redis)?;

    // ── 1. Check for an existing completed record ────────────────────────────
    let existing: Option<String> = conn.get(&record_key).await.map_err(AppError::Redis)?;

    if let Some(json) = existing {
        let record: StoredRecord = serde_json::from_str(&json)
            .map_err(|e| AppError::Internal(format!("idempotency decode: {e}")))?;

        if record.body_hash != hash {
            return Err(AppError::Conflict(
                "Idempotency key already used with a different request body".into(),
                vec![],
            ));
        }

        let cached: T = serde_json::from_value(record.response)
            .map_err(|e| AppError::Internal(format!("idempotency response decode: {e}")))?;
        return Ok(cached);
    }

    // ── 2. Acquire distributed lock ──────────────────────────────────────────
    let lock_token = uuid::Uuid::new_v4().to_string();
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

    // ── 3. Execute ───────────────────────────────────────────────────────────
    let result = f().await;

    // ── 4. Release lock safely via Lua (only delete if token still matches) ──
    let release_script = r#"
        if redis.call("get", KEYS[1]) == ARGV[1] then
            return redis.call("del", KEYS[1])
        else
            return 0
        end
    "#;
    let _: i64 = redis::Script::new(release_script)
        .key(&lock_key)
        .arg(&lock_token)
        .invoke_async(&mut conn)
        .await
        .unwrap_or(0);

    // ── 5. On success, persist the response; on error leave no record ────────
    match result {
        Ok(response) => {
            let response_val = serde_json::to_value(&response).unwrap_or(serde_json::Value::Null);
            let record = StoredRecord {
                body_hash: hash.to_owned(),
                response: response_val,
            };
            let _: () = conn
                .set_ex(
                    &record_key,
                    serde_json::to_string(&record).unwrap(),
                    RECORD_TTL_SECS,
                )
                .await
                .unwrap_or(());
            Ok(response)
        }
        Err(e) => Err(e),
    }
}
