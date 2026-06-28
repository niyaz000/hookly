use uuid::Uuid;

fn open_key(endpoint_id: Uuid) -> String {
    format!("hookly:cb:open:{endpoint_id}")
}

fn fail_key(endpoint_id: Uuid) -> String {
    format!("hookly:cb:fail:{endpoint_id}")
}

/// Returns true if the circuit is open (endpoint is unhealthy, skip delivery).
/// Fails open (returns false) on Redis errors so a blip doesn't halt delivery.
pub async fn is_open(client: &redis::Client, endpoint_id: Uuid) -> bool {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(endpoint_id = %endpoint_id, "circuitbreaker: connect failed: {e}");
            return false;
        }
    };
    redis::cmd("EXISTS")
        .arg(open_key(endpoint_id))
        .query_async::<i64>(&mut conn)
        .await
        .unwrap_or(0)
        > 0
}

/// Records a delivery failure and returns true if the failure count crossed the
/// threshold within the window (i.e. the circuit should now be opened).
/// Fails closed (returns false) on Redis errors.
pub async fn record_failure(
    client: &redis::Client,
    endpoint_id: Uuid,
    threshold: u32,
    window_secs: u64,
) -> bool {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(endpoint_id = %endpoint_id, "circuitbreaker: connect failed: {e}");
            return false;
        }
    };

    let script = redis::Script::new(
        r"
        local key    = KEYS[1]
        local window = tonumber(ARGV[1])
        local count  = redis.call('INCR', key)
        if count == 1 then redis.call('EXPIRE', key, window) end
        return count
        ",
    );

    let count: i64 = script
        .key(fail_key(endpoint_id))
        .arg(window_secs)
        .invoke_async(&mut conn)
        .await
        .unwrap_or(0);

    count >= threshold as i64
}

/// Opens the circuit for `cooldown_secs`. After the TTL expires the circuit is
/// implicitly half-open: the next delivery attempt acts as a probe.
pub async fn open_circuit(client: &redis::Client, endpoint_id: Uuid, cooldown_secs: u64) {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(endpoint_id = %endpoint_id, "circuitbreaker: connect failed: {e}");
            return;
        }
    };
    let _: redis::RedisResult<()> = redis::cmd("SET")
        .arg(open_key(endpoint_id))
        .arg(1i64)
        .arg("EX")
        .arg(cooldown_secs)
        .query_async(&mut conn)
        .await;
}

/// Clears failure state after a successful delivery (closed → reset).
pub async fn reset(client: &redis::Client, endpoint_id: Uuid) {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(endpoint_id = %endpoint_id, "circuitbreaker: connect failed: {e}");
            return;
        }
    };
    let _: redis::RedisResult<()> = redis::cmd("DEL")
        .arg(open_key(endpoint_id))
        .arg(fail_key(endpoint_id))
        .query_async(&mut conn)
        .await;
}
