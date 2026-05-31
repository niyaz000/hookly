use std::time::Duration;

#[derive(Clone, Debug)]
pub struct WorkerConfig {
    /// Streams this worker instance will consume, derived from WORKER_STREAMS.
    pub streams: Vec<String>,
    /// Messages to fetch per XREADGROUP call.
    pub batch_size: i64,
    /// Unique name for this consumer within the group.
    pub consumer_name: String,
    /// How long to block on XREADGROUP before looping to check shutdown flag.
    pub block_ms: i64,
    /// Idle threshold for XAUTOCLAIM — reclaim messages held longer than this.
    pub reclaim_idle_ms: i64,
    /// Interval between outbox scans.
    pub outbox_interval: Duration,
    /// HTTP timeout for delivery attempts.
    pub delivery_timeout: Duration,
}

impl WorkerConfig {
    pub fn from_env() -> Self {
        let streams_raw = std::env::var("WORKER_STREAMS")
            .unwrap_or_else(|_| hookly::queue::STREAM_DEFAULT.to_string());
        let streams = streams_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let consumer_name = std::env::var("WORKER_CONSUMER_NAME").unwrap_or_else(|_| {
            let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "worker".to_string());
            format!("{}-{}", hostname, std::process::id())
        });

        Self {
            streams,
            batch_size: parse_env("WORKER_BATCH_SIZE", 10),
            consumer_name,
            block_ms: parse_env("WORKER_BLOCK_MS", 5_000),
            reclaim_idle_ms: parse_env("WORKER_RECLAIM_IDLE_MS", 90_000),
            outbox_interval: Duration::from_secs(parse_env("WORKER_OUTBOX_INTERVAL_SECS", 10)),
            delivery_timeout: Duration::from_secs(parse_env("WORKER_DELIVERY_TIMEOUT_SECS", 30)),
        }
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
