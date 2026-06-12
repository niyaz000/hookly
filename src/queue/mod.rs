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

/// Non-blocking read of up to `count` messages from a single stream.
///
/// Returns immediately whether or not messages are available — callers use this
/// to implement round-robin across streams without blocking on any one of them.
pub async fn xreadgroup_single(
    client: &redis::Client,
    stream: &str,
    consumer_name: &str,
    count: i64,
) -> Vec<(String, String)> {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("xreadgroup_single: connect failed: {e}");
            return vec![];
        }
    };

    // No BLOCK argument → non-blocking; returns immediately if stream is empty.
    let val: redis::RedisResult<redis::Value> = redis::cmd("XREADGROUP")
        .arg("GROUP")
        .arg(GROUP)
        .arg(consumer_name)
        .arg("COUNT")
        .arg(count)
        .arg("STREAMS")
        .arg(stream)
        .arg(">")
        .query_async(&mut conn)
        .await;

    match val {
        Ok(v) => parse_xread_reply(v),
        Err(e) if e.to_string().contains("NOGROUP") => {
            tracing::warn!("xreadgroup_single: NOGROUP for {stream}");
            vec![]
        }
        Err(e) => {
            tracing::warn!("xreadgroup_single error on {stream}: {e}");
            vec![]
        }
    }
}

/// Reads up to `count` messages from every stream in `streams` in one
/// XREADGROUP call, blocking for at most `block_ms`.
///
/// Returns `(stream_name, msg_id, job_public_id)` tuples. Redis delivers from
/// whichever stream has messages — empty streams are transparently skipped.
/// N workers calling this simultaneously get naturally load-balanced by the
/// consumer group; each message is delivered to exactly one consumer.
pub async fn xreadgroup_multi(
    client: &redis::Client,
    streams: &[String],
    consumer_name: &str,
    count: i64,
    block_ms: i64,
) -> Vec<(String, String, String)> {
    if streams.is_empty() {
        return vec![];
    }

    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("xreadgroup_multi: connect failed: {e}");
            return vec![];
        }
    };

    let mut cmd = redis::cmd("XREADGROUP");
    cmd.arg("GROUP")
        .arg(GROUP)
        .arg(consumer_name)
        .arg("COUNT")
        .arg(count)
        .arg("BLOCK")
        .arg(block_ms)
        .arg("STREAMS");
    for s in streams {
        cmd.arg(s.as_str());
    }
    for _ in streams {
        cmd.arg(">");
    }

    let val: redis::RedisResult<redis::Value> = cmd.query_async(&mut conn).await;

    match val {
        Ok(v) => parse_xread_multi_reply(v),
        Err(e) if e.to_string().contains("NOGROUP") => {
            tracing::warn!("xreadgroup_multi: NOGROUP on one of {:?}", streams);
            vec![]
        }
        Err(e) => {
            tracing::warn!("xreadgroup_multi error: {e}");
            vec![]
        }
    }
}

/// Trims a stream without risking data loss.
///
/// When the PEL is non-empty: trim everything before the oldest pending entry
/// (all prior entries are guaranteed to be ACK'd — Redis delivers in order).
///
/// When the PEL is empty: trim up to the group's `last-delivered-id` (all
/// messages have been consumed and ACK'd).
pub async fn xtrim_safe(client: &redis::Client, stream: &str) {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("xtrim_safe: connect failed for {stream}: {e}");
            return;
        }
    };

    let Some(trim_id) = safe_trim_id(&mut conn, stream).await else {
        return;
    };
    if trim_id == "0-0" || trim_id.is_empty() {
        return;
    }

    let result: redis::RedisResult<i64> = redis::cmd("XTRIM")
        .arg(stream)
        .arg("MINID")
        .arg("~")
        .arg(&trim_id)
        .query_async(&mut conn)
        .await;

    match result {
        Ok(n) if n > 0 => tracing::info!(stream, trimmed = n, cutoff = %trim_id, "stream trimmed"),
        Ok(_) => {}
        Err(e) => tracing::warn!("xtrim failed for {stream}: {e}"),
    }
}

/// Scans Redis for all stream keys matching `pattern` (e.g. `"hookly:q:*"`).
/// Used by the stream-watcher task to auto-discover new enterprise streams.
pub async fn scan_streams(client: &redis::Client, pattern: &str) -> Vec<String> {
    let mut conn = match client.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("scan_streams: connect failed: {e}");
            return vec![];
        }
    };

    let mut found = Vec::new();
    let mut cursor: u64 = 0;

    loop {
        let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("TYPE")
            .arg("stream")
            .arg("COUNT")
            .arg(100i64)
            .query_async(&mut conn)
            .await
            .unwrap_or((0, vec![]));

        found.extend(keys);
        cursor = next;
        if cursor == 0 {
            break;
        }
    }

    found
}

// --- Internal helpers ---

/// Returns the safe MINID to use for XTRIM:
/// - PEL non-empty → oldest PEL entry (everything before it is ACK'd)
/// - PEL empty → last-delivered-id from XINFO GROUPS (all consumed + ACK'd)
async fn safe_trim_id(
    conn: &mut redis::aio::MultiplexedConnection,
    stream: &str,
) -> Option<String> {
    // XPENDING stream group — summary form → [count, min-id, max-id, consumers]
    let pending: redis::RedisResult<redis::Value> = redis::cmd("XPENDING")
        .arg(stream)
        .arg(GROUP)
        .query_async(conn)
        .await;

    match pending {
        Ok(redis::Value::Array(ref parts)) if parts.len() >= 2 => {
            match &parts[0] {
                redis::Value::Int(0) => xinfo_last_delivered(conn, stream).await,
                _ => redis_str(&parts[1]),
            }
        }
        _ => None,
    }
}

/// Returns the `last-delivered-id` for the "workers" consumer group via XINFO GROUPS.
async fn xinfo_last_delivered(
    conn: &mut redis::aio::MultiplexedConnection,
    stream: &str,
) -> Option<String> {
    let val: redis::RedisResult<redis::Value> = redis::cmd("XINFO")
        .arg("GROUPS")
        .arg(stream)
        .query_async(conn)
        .await;

    let groups = match val {
        Ok(redis::Value::Array(g)) => g,
        _ => return None,
    };

    for group in groups {
        // Each group is a flat alternating key-value array (RESP2).
        let fields = match group {
            redis::Value::Array(f) => f,
            _ => continue,
        };
        let mut i = 0;
        let mut is_workers = false;
        let mut last_id: Option<String> = None;
        while i + 1 < fields.len() {
            match redis_str(&fields[i]).as_deref() {
                Some("name") => {
                    is_workers = redis_str(&fields[i + 1]).as_deref() == Some(GROUP);
                }
                Some("last-delivered-id") => {
                    last_id = redis_str(&fields[i + 1]);
                }
                _ => {}
            }
            i += 2;
        }
        if is_workers {
            return last_id;
        }
    }
    None
}

fn parse_xread_multi_reply(val: redis::Value) -> Vec<(String, String, String)> {
    // XREADGROUP multi-stream: [[stream_name, [[msg_id, fields], ...]], ...]
    let stream_list = match val {
        redis::Value::Array(v) => v,
        redis::Value::Nil => return vec![],
        _ => return vec![],
    };

    let mut result = Vec::new();
    for stream_entry in stream_list {
        let parts = match stream_entry {
            redis::Value::Array(p) => p,
            _ => continue,
        };
        if parts.len() < 2 {
            continue;
        }
        let stream_name = match redis_str(&parts[0]) {
            Some(s) => s,
            None => continue,
        };
        let messages = match &parts[1] {
            redis::Value::Array(m) => m.clone(),
            redis::Value::Nil => continue,
            _ => continue,
        };
        for entry in messages {
            let entry_parts = match entry {
                redis::Value::Array(p) => p,
                _ => continue,
            };
            if entry_parts.len() < 2 {
                continue;
            }
            let msg_id = match redis_str(&entry_parts[0]) {
                Some(id) => id,
                None => continue,
            };
            let job_id = match extract_field_j(&entry_parts[1]) {
                Some(j) => j,
                None => continue,
            };
            result.push((stream_name.clone(), msg_id, job_id));
        }
    }
    result
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
