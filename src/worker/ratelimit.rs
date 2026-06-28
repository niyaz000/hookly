use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn blocked_key(endpoint_id: Uuid) -> String {
    format!("hookly:rl:blocked:{endpoint_id}")
}

fn counter_key(endpoint_id: Uuid) -> String {
    let minute = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 60;
    format!("hookly:rl:cnt:{endpoint_id}:{minute}")
}

/// Returns true if the endpoint has an active block key (set after a 429).
/// Fails open (returns false) on Redis errors so a connectivity blip doesn't stall all delivery.
pub async fn is_blocked(client: &redis::Client, endpoint_id: Uuid) -> bool {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(endpoint_id = %endpoint_id, "ratelimit: connect failed: {e}");
            return false;
        }
    };
    redis::cmd("EXISTS")
        .arg(blocked_key(endpoint_id))
        .query_async::<i64>(&mut conn)
        .await
        .unwrap_or(0)
        > 0
}

/// Atomically increments the per-minute counter and returns true if the call is
/// allowed (counter ≤ limit). If over limit the increment is rolled back so the
/// slot isn't consumed. Fails open on Redis errors.
pub async fn try_acquire(client: &redis::Client, endpoint_id: Uuid, limit: i32) -> bool {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(endpoint_id = %endpoint_id, "ratelimit: connect failed: {e}");
            return true;
        }
    };

    // INCR then rollback if over limit. TTL of 90s (> 60s window) ensures the
    // key is gone well before the next minute bucket would alias to it.
    let script = redis::Script::new(
        r"
        local key   = KEYS[1]
        local limit = tonumber(ARGV[1])
        local count = redis.call('INCR', key)
        if count == 1 then redis.call('EXPIRE', key, 90) end
        if count > limit then
            redis.call('DECR', key)
            return 0
        end
        return 1
        ",
    );

    script
        .key(counter_key(endpoint_id))
        .arg(limit)
        .invoke_async::<i64>(&mut conn)
        .await
        .unwrap_or(1)
        != 0
}

/// Marks an endpoint as blocked for `ttl_secs` seconds.
/// Called when the endpoint returns 429.
pub async fn set_blocked(client: &redis::Client, endpoint_id: Uuid, ttl_secs: u64) {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(endpoint_id = %endpoint_id, "ratelimit: connect failed: {e}");
            return;
        }
    };
    let _: redis::RedisResult<()> = redis::cmd("SET")
        .arg(blocked_key(endpoint_id))
        .arg(1i64)
        .arg("EX")
        .arg(ttl_secs)
        .query_async(&mut conn)
        .await;
}
