use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SchedulerConfig {
    /// Number of concurrent worker tasks. Each task competes for shards from
    /// sched:shards on every iteration — no static shard assignment.
    pub worker_count: u8,
    /// How long a worker sleeps when sched:shards is empty before retrying.
    pub idle_sleep: Duration,
    /// How often the reconciliation task re-syncs DB → Redis sorted sets.
    pub reconcile_interval: Duration,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            worker_count: env_u8("SCHEDULER_WORKER_COUNT", 4),
            idle_sleep: Duration::from_millis(env_u64("SCHEDULER_IDLE_SLEEP_MS", 1_000)),
            reconcile_interval: Duration::from_millis(env_u64("SCHEDULER_RECONCILE_MS", 120_000)),
        }
    }
}

fn env_u8(key: &str, default: u8) -> u8 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
