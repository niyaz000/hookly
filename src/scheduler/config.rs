use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SchedulerConfig {
    /// Number of shards to manage. Each shard is an independent sorted set in Redis.
    pub shard_count: i16,
    /// How often each owned shard checks for due schedules.
    pub tick_interval: Duration,
    /// How often the heartbeat key is renewed (must be < shard ownership TTL of 30s).
    pub heartbeat_interval: Duration,
    /// How often the reconciliation task re-syncs DB → Redis sorted sets.
    pub reconcile_interval: Duration,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            shard_count: env_i16("SCHEDULER_SHARD_COUNT", 4),
            tick_interval: Duration::from_millis(env_u64("SCHEDULER_TICK_MS", 5_000)),
            heartbeat_interval: Duration::from_millis(env_u64("SCHEDULER_HEARTBEAT_MS", 10_000)),
            reconcile_interval: Duration::from_millis(env_u64("SCHEDULER_RECONCILE_MS", 120_000)),
        }
    }
}

fn env_i16(key: &str, default: i16) -> i16 {
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
