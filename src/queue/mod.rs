use uuid::Uuid;

pub const GROUP: &str = "workers";

pub const STREAM_DEFAULT: &str = "hookly:q:tier:default";
pub const STREAM_SILVER: &str = "hookly:q:tier:silver";
pub const STREAM_GOLD: &str = "hookly:q:tier:gold";
pub const STREAM_PLATINUM: &str = "hookly:q:tier:platinum";

/// All shared tier streams. Ensure these groups exist at API + worker startup.
pub const TIER_STREAMS: &[&str] = &[STREAM_DEFAULT, STREAM_SILVER, STREAM_GOLD, STREAM_PLATINUM];

/// Resolves the stream name for an org based on its tier.
///
/// Enterprise orgs get a dedicated stream scoped to their org UUID so they
/// share no queue capacity with other tenants.
pub fn stream_for_tier(tier: &str, org_id: Uuid) -> String {
    match tier {
        "enterprise" => format!("hookly:q:org:{}", org_id),
        t => format!("hookly:q:tier:{}", t),
    }
}

/// Idempotently ensures a consumer group exists for `stream`.
///
/// `start_id`:
/// - `"$"` — only consume messages produced after the group is created (use
///   for shared tier streams created at startup, where prior messages were
///   already delivered by other workers).
/// - `"0-0"` — consume all existing messages too (use for enterprise streams
///   whose dedicated worker may start after messages were already enqueued).
pub async fn ensure_consumer_group(
    client: &redis::Client,
    stream: &str,
    start_id: &str,
) -> Result<(), redis::RedisError> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let result: redis::RedisResult<()> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(stream)
        .arg(GROUP)
        .arg(start_id)
        .arg("MKSTREAM")
        .query_async(&mut conn)
        .await;

    match result {
        Ok(_) => Ok(()),
        Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
        Err(e) => Err(e),
    }
}

/// Appends a delivery-job reference to a Redis Stream.
/// Only the public_id is stored under field "j" to keep entries small.
pub async fn enqueue(
    client: &redis::Client,
    stream: &str,
    job_public_id: &str,
) -> Result<(), redis::RedisError> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let _: String = redis::cmd("XADD")
        .arg(stream)
        .arg("*")
        .arg("j")
        .arg(job_public_id)
        .query_async(&mut conn)
        .await?;
    Ok(())
}

/// Acknowledges a message, removing it from the consumer's PEL.
pub async fn xack(
    client: &redis::Client,
    stream: &str,
    msg_id: &str,
) -> Result<(), redis::RedisError> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    redis::cmd("XACK")
        .arg(stream)
        .arg(GROUP)
        .arg(msg_id)
        .query_async::<()>(&mut conn)
        .await
}

/// Claims idle messages from other (possibly crashed) consumers and returns
/// them as `(msg_id, job_public_id)` pairs for immediate reprocessing.
///
/// `min_idle_ms` — only claim messages idle longer than this (e.g. 90_000 for 90s).
pub async fn xautoclaim(
    client: &redis::Client,
    stream: &str,
    consumer_name: &str,
    min_idle_ms: i64,
) -> Vec<(String, String)> {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("xautoclaim: failed to connect: {e}");
            return vec![];
        }
    };

    let val: redis::RedisResult<redis::Value> = redis::cmd("XAUTOCLAIM")
        .arg(stream)
        .arg(GROUP)
        .arg(consumer_name)
        .arg(min_idle_ms)
        .arg("0-0")
        .arg("COUNT")
        .arg(100i64)
        .query_async(&mut conn)
        .await;

    match val {
        Ok(v) => parse_autoclaim_entries(v),
        Err(e) => {
            tracing::warn!("xautoclaim failed on {stream}: {e}");
            vec![]
        }
    }
}

// --- Internal parsing helpers ---

/// Parses a raw XREADGROUP reply into `(msg_id, job_public_id)` pairs.
/// Returns an empty vec on any unexpected structure.
pub fn parse_xread_reply(val: redis::Value) -> Vec<(String, String)> {
    // XREADGROUP returns: [[stream_name, [[msg_id, [fields...]], ...]]]
    let streams = match val {
        redis::Value::Array(v) => v,
        redis::Value::Nil => return vec![],
        _ => return vec![],
    };

    let first_stream = match streams.into_iter().next() {
        Some(redis::Value::Array(s)) => s,
        _ => return vec![],
    };

    // first_stream = [stream_name, messages_array]
    let messages = match first_stream.into_iter().nth(1) {
        Some(redis::Value::Array(m)) => m,
        _ => return vec![],
    };

    parse_entries(messages)
}

fn parse_autoclaim_entries(val: redis::Value) -> Vec<(String, String)> {
    // XAUTOCLAIM returns: [next_cursor, [[msg_id, fields...], ...], [deleted_ids]]
    let parts = match val {
        redis::Value::Array(v) => v,
        _ => return vec![],
    };

    let entries = match parts.into_iter().nth(1) {
        Some(redis::Value::Array(e)) => e,
        _ => return vec![],
    };

    parse_entries(entries)
}

fn parse_entries(entries: Vec<redis::Value>) -> Vec<(String, String)> {
    entries
        .into_iter()
        .filter_map(|entry| {
            let parts = match entry {
                redis::Value::Array(p) => p,
                _ => return None,
            };
            if parts.len() < 2 {
                return None;
            }
            let msg_id = redis_str(&parts[0])?;
            let job_id = extract_field_j(&parts[1])?;
            Some((msg_id, job_id))
        })
        .collect()
}

fn redis_str(val: &redis::Value) -> Option<String> {
    match val {
        redis::Value::BulkString(b) => String::from_utf8(b.clone()).ok(),
        redis::Value::SimpleString(s) => Some(s.clone()),
        _ => None,
    }
}

fn extract_field_j(val: &redis::Value) -> Option<String> {
    let fields = match val {
        redis::Value::Array(f) => f,
        _ => return None,
    };
    let mut i = 0;
    while i + 1 < fields.len() {
        if redis_str(&fields[i]).as_deref() == Some("j") {
            return redis_str(&fields[i + 1]);
        }
        i += 2;
    }
    None
}
