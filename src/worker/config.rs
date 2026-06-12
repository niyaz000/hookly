use std::time::Duration;

#[derive(Clone, Debug)]
pub struct WorkerConfig {
    /// Streams this worker instance will consume, derived from WORKER_STREAMS.
    pub streams: Vec<String>,
    /// Number of concurrent worker tasks per pod.
    pub num_workers: usize,
    /// Messages to fetch per XREADGROUP call.
    pub batch_size: i64,
    /// Unique name for this consumer within the group (each task appends "-wN").
    pub consumer_name: String,
    /// Idle threshold for XAUTOCLAIM — reclaim messages held longer than this.
    pub reclaim_idle_ms: i64,
    /// Interval between outbox scans.
    pub outbox_interval: Duration,
    /// HTTP timeout for delivery attempts.
    pub delivery_timeout: Duration,
    /// How often the stream-watcher scans Redis for new streams (seconds).
    pub stream_watch_interval_secs: u64,
    /// How often the trimmer runs safe XTRIM on each stream (seconds).
    pub trim_interval_secs: u64,
    /// How long to sleep (ms) after a full rotation returns no messages.
    /// Controls idle CPU usage vs. message latency when all streams are quiet.
    pub poll_interval_ms: u64,
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
            num_workers: parse_env("WORKER_NUM_WORKERS", 4usize),
            batch_size: parse_env("WORKER_BATCH_SIZE", 10),
            consumer_name,
            reclaim_idle_ms: parse_env("WORKER_RECLAIM_IDLE_MS", 90_000),
            outbox_interval: Duration::from_secs(parse_env("WORKER_OUTBOX_INTERVAL_SECS", 10)),
            delivery_timeout: Duration::from_secs(parse_env("WORKER_DELIVERY_TIMEOUT_SECS", 30)),
            stream_watch_interval_secs: parse_env("WORKER_STREAM_WATCH_INTERVAL_SECS", 30u64),
            trim_interval_secs: parse_env("WORKER_TRIM_INTERVAL_SECS", 60u64),
            poll_interval_ms: parse_env("WORKER_POLL_INTERVAL_MS", 250u64),
        }
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
