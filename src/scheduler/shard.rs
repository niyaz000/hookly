use chrono::Utc;
use redis::AsyncCommands;
use tokio::sync::watch;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::config::SchedulerConfig;
use super::fire::fire_schedule;

const OWNERSHIP_TTL_SECS: u64 = 30;

/// Ownership + tick loop for a single shard.
///
/// Each instance competes for ownership of the shard via a Redis NX key.
/// Only the owner runs the tick loop. All instances attempt to claim on every
/// heartbeat so failover is bounded by `heartbeat_interval`.
pub async fn run(
    shard_id: i16,
    instance_id: String,
    cfg: SchedulerConfig,
    db: sqlx::PgPool,
    redis: redis::Client,
    mut shutdown: watch::Receiver<bool>,
) {
    info!(shard = shard_id, instance = %instance_id, "shard task started");

    let owner_key = format!("sched:owner:{shard_id}");
    let pending_key = format!("sched:pending:{shard_id}");

    let mut tick = tokio::time::interval(cfg.tick_interval);
    let mut heartbeat = tokio::time::interval(cfg.heartbeat_interval);

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!(shard = shard_id, "shard task shutting down");
                    return;
                }
            }
            _ = heartbeat.tick() => {
                // Renew ownership unconditionally. If we lost the key it's a re-claim attempt.
                if let Ok(mut conn) = redis.get_multiplexed_async_connection().await {
                    let _: Result<(), _> = redis::cmd("SET")
                        .arg(&owner_key)
                        .arg(&instance_id)
                        .arg("EX")
                        .arg(OWNERSHIP_TTL_SECS)
                        .query_async(&mut conn)
                        .await;
                }
            }
            _ = tick.tick() => {
                if !is_owner(&redis, &owner_key, &instance_id).await {
                    // Try to claim.
                    if !try_claim(&redis, &owner_key, &instance_id).await {
                        debug!(shard = shard_id, "not owner, skipping tick");
                        continue;
                    }
                    info!(shard = shard_id, "claimed shard ownership");
                }

                run_tick(shard_id, &pending_key, &db, &redis).await;
            }
        }
    }
}

async fn is_owner(redis: &redis::Client, owner_key: &str, instance_id: &str) -> bool {
    let Ok(mut conn) = redis.get_multiplexed_async_connection().await else {
        return false;
    };
    let owner: Option<String> = conn.get(owner_key).await.unwrap_or(None);
    owner.as_deref() == Some(instance_id)
}

async fn try_claim(redis: &redis::Client, owner_key: &str, instance_id: &str) -> bool {
    let Ok(mut conn) = redis.get_multiplexed_async_connection().await else {
        return false;
    };
    let result: Option<String> = redis::cmd("SET")
        .arg(owner_key)
        .arg(instance_id)
        .arg("NX")
        .arg("EX")
        .arg(OWNERSHIP_TTL_SECS)
        .query_async(&mut conn)
        .await
        .unwrap_or(None);
    result.is_some()
}

async fn run_tick(
    shard_id: i16,
    pending_key: &str,
    db: &sqlx::PgPool,
    redis: &redis::Client,
) {
    let now_score = Utc::now().timestamp() as f64;

    let schedule_ids: Vec<String> = {
        let Ok(mut conn) = redis.get_multiplexed_async_connection().await else {
            warn!(shard = shard_id, "redis unavailable during tick");
            return;
        };
        redis::cmd("ZRANGEBYSCORE")
            .arg(pending_key)
            .arg(0.0)
            .arg(now_score)
            .arg("LIMIT")
            .arg(0)
            .arg(500)
            .query_async(&mut conn)
            .await
            .unwrap_or_default()
    };

    if schedule_ids.is_empty() {
        return;
    }

    debug!(shard = shard_id, count = schedule_ids.len(), "due schedules found");

    for id_str in &schedule_ids {
        let schedule_id = match Uuid::parse_str(id_str) {
            Ok(id) => id,
            Err(_) => {
                warn!(shard = shard_id, id = %id_str, "invalid UUID in sorted set");
                continue;
            }
        };

        // Fire lock: prevents duplicate fires across scheduler instances that may
        // both claim ownership during a failover window.
        let minute_bucket = Utc::now().timestamp() / 60;
        let fire_key = format!("sched:fire:{schedule_id}:{minute_bucket}");

        let acquired = {
            let Ok(mut conn) = redis.get_multiplexed_async_connection().await else {
                warn!(shard = shard_id, "redis unavailable for fire lock");
                continue;
            };
            let result: Option<String> = redis::cmd("SET")
                .arg(&fire_key)
                .arg("1")
                .arg("NX")
                .arg("EX")
                .arg(120u64)
                .query_async(&mut conn)
                .await
                .unwrap_or(None);
            result.is_some()
        };

        if !acquired {
            debug!(schedule_id = %schedule_id, "fire lock already held, skipping");
            continue;
        }

        fire_schedule(schedule_id, db, redis).await;
    }
}
