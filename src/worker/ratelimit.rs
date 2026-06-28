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

/// Returns the remaining TTL (seconds) on the block key if the endpoint is blocked,
/// or None if it is not blocked. Fails open (returns None) on Redis errors.
pub async fn blocked_remaining_secs(client: &redis::Client, endpoint_id: Uuid) -> Option<u64> {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(endpoint_id = %endpoint_id, "ratelimit: connect failed: {e}");
            return None;
        }
    };
    // TTL returns: positive = remaining seconds, -1 = no TTL, -2 = key absent
    match redis::cmd("TTL")
        .arg(blocked_key(endpoint_id))
        .query_async::<i64>(&mut conn)
        .await
        .unwrap_or(-2)
    {
        t if t > 0 => Some(t as u64),
        _ => None,
    }
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
