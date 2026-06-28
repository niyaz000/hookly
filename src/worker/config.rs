use std::time::Duration;

#[derive(Clone, Debug)]
pub struct WorkerConfig {
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
    /// Maximum concurrent in-flight delivery tasks across all workers in this pod.
    pub max_inflight: usize,
    /// How often the trimmer runs safe XTRIM on each stream (seconds).
    pub trim_interval_secs: u64,
    /// How long to sleep (ms) when the scheduling sorted set is empty.
    pub poll_interval_ms: u64,
    /// Number of endpoint failures within `cb_window_secs` that trips the circuit.
    pub cb_failure_threshold: u32,
    /// Sliding window (seconds) over which failures are counted.
    pub cb_window_secs: u64,
    /// How long (seconds) the circuit stays open before allowing a probe attempt.
    pub cb_cooldown_secs: u64,
}

impl WorkerConfig {
    pub fn from_env() -> Self {
        let consumer_name = std::env::var("WORKER_CONSUMER_NAME").unwrap_or_else(|_| {
            let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "worker".to_string());
            format!("{}-{}", hostname, std::process::id())
        });

        Self {
            num_workers: parse_env("WORKER_NUM_WORKERS", 4usize),
            batch_size: parse_env("WORKER_BATCH_SIZE", 10),
            consumer_name,
            reclaim_idle_ms: parse_env("WORKER_RECLAIM_IDLE_MS", 90_000),
            outbox_interval: Duration::from_secs(parse_env("WORKER_OUTBOX_INTERVAL_SECS", 10)),
            delivery_timeout: Duration::from_secs(parse_env("WORKER_DELIVERY_TIMEOUT_SECS", 10)),
            max_inflight: parse_env("WORKER_MAX_INFLIGHT", 64usize),
            trim_interval_secs: parse_env("WORKER_TRIM_INTERVAL_SECS", 60u64),
            poll_interval_ms: parse_env("WORKER_POLL_INTERVAL_MS", 250u64),
            cb_failure_threshold: parse_env("WORKER_CB_FAILURE_THRESHOLD", 5u32),
            cb_window_secs: parse_env("WORKER_CB_WINDOW_SECS", 60u64),
            cb_cooldown_secs: parse_env("WORKER_CB_COOLDOWN_SECS", 30u64),
        }
    }
}

fn parse_env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
