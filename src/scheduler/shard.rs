use chrono::Utc;
use tokio::sync::watch;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::config::SchedulerConfig;
use super::fire::fire_schedule;

pub const SHARDS_KEY: &str = "sched:shards";
const LOCK_TTL_SECS: u64 = 30;

// Removes shard_id from sched:shards only if its score matches the value read
// before the empty check. If the API bumped the score (new schedule added to
// this shard), this is a no-op — the shard stays in the discovery set.
const REMOVE_IF_SCORE_MATCHES: &str = r#"
local current = redis.call('ZSCORE', KEYS[1], ARGV[1])
if current and tonumber(current) == tonumber(ARGV[2]) then
  redis.call('ZREM', KEYS[1], ARGV[1])
  return 1
end
return 0
"#;

// Releases the per-shard lock only if we are still the owner. Guards against
// releasing a lock that has already expired and been claimed by another worker.
const RELEASE_LOCK_LUA: &str = r#"
if redis.call('get', KEYS[1]) == ARGV[1] then
  redis.call('del', KEYS[1])
  return 1
end
return 0
"#;

/// Worker task: continuously picks a random active shard, acquires its lock,
/// fires any due schedules, then releases immediately.
///
/// N of these tasks run per scheduler instance (SCHEDULER_WORKER_COUNT). They
/// all compete via ZRANDMEMBER on sched:shards — no static shard assignment.
pub async fn run(
    instance_id: String,
    cfg: SchedulerConfig,
    db: sqlx::PgPool,
    redis: redis::Client,
    mut shutdown: watch::Receiver<bool>,
) {
    info!(instance = %instance_id, "worker task started");

    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!(instance = %instance_id, "worker task shutting down");
                    return;
                }
            }
            _ = process_one(&instance_id, &cfg, &db, &redis) => {}
        }
    }
}

async fn process_one(
    instance_id: &str,
    cfg: &SchedulerConfig,
    db: &sqlx::PgPool,
    redis: &redis::Client,
) {
    let Ok(mut conn) = redis.get_multiplexed_async_connection().await else {
        warn!("worker: redis unavailable");
        tokio::time::sleep(cfg.idle_sleep).await;
        return;
    };

    // 1. Pick a random active shard from the discovery set.
    let shard_str: Option<String> = redis::cmd("ZRANDMEMBER")
        .arg(SHARDS_KEY)
        .query_async(&mut conn)
        .await
        .unwrap_or(None);

    let shard_str = match shard_str {
        Some(s) => s,
        None => {
            // No shards in the set — all schedules are either idle or don't exist yet.
            debug!("sched:shards empty, sleeping");
            tokio::time::sleep(cfg.idle_sleep).await;
            return;
        }
    };

    let shard_id: i16 = match shard_str.parse() {
        Ok(id) => id,
        Err(_) => {
            warn!(shard = %shard_str, "invalid shard id in sched:shards");
            return;
        }
    };

    // 2. Read the shard's current score before the empty check.
    //    This score is the "version" used for race-safe removal below.
    let score: Option<f64> = redis::cmd("ZSCORE")
        .arg(SHARDS_KEY)
        .arg(&shard_str)
        .query_async(&mut conn)
        .await
        .unwrap_or(None);

    let score = match score {
        Some(s) => s,
        None => return, // shard vanished between ZRANDMEMBER and ZSCORE
    };

    // 3. Try to acquire the per-shard lock (NX = only if not already held).
    let lock_key = format!("sched:lock:{shard_id}");
    let acquired: Option<String> = redis::cmd("SET")
        .arg(&lock_key)
        .arg(instance_id)
        .arg("NX")
        .arg("EX")
        .arg(LOCK_TTL_SECS)
        .query_async(&mut conn)
        .await
        .unwrap_or(None);

    if acquired.is_none() {
        debug!(shard = shard_id, "lock held by another worker, skipping");
        return;
    }

    // 4. Poll for schedules due now (up to 50 per tick).
    let pending_key = format!("sched:pending:{shard_id}");
    let now_score = Utc::now().timestamp() as f64;

    let schedule_ids: Vec<String> = redis::cmd("ZRANGEBYSCORE")
        .arg(&pending_key)
        .arg(0.0)
        .arg(now_score)
        .arg("LIMIT")
        .arg(0)
        .arg(50)
        .query_async(&mut conn)
        .await
        .unwrap_or_default();

    if schedule_ids.is_empty() {
        // Shard is idle. Remove it from the discovery set, but only if the score
        // hasn't changed since we read it at step 2. If the API added a new
        // schedule after step 2 it will have bumped the score via ZADD GT,
        // making the Lua check fail — the shard stays in the set.
        let _: Result<i64, _> = redis::Script::new(REMOVE_IF_SCORE_MATCHES)
            .key(SHARDS_KEY)
            .arg(&shard_str)
            .arg(score)
            .invoke_async(&mut conn)
            .await;

        release_lock(&mut conn, &lock_key, instance_id).await;
        return;
    }

    debug!(shard = shard_id, count = schedule_ids.len(), "firing due schedules");

    // Drop the multiplexed connection before firing so fire_schedule can use
    // its own connections without contending on this one.
    drop(conn);

    for id_str in &schedule_ids {
        let schedule_id = match Uuid::parse_str(id_str) {
            Ok(id) => id,
            Err(_) => {
                warn!(shard = shard_id, id = %id_str, "invalid UUID in sched:pending");
                continue;
            }
        };
        fire_schedule(schedule_id, db, redis).await;
    }

    // 5. Release lock immediately after the batch is complete.
    if let Ok(mut conn) = redis.get_multiplexed_async_connection().await {
        release_lock(&mut conn, &lock_key, instance_id).await;
    }
    // If Redis is unavailable here the lock expires naturally via TTL (30s).
}

async fn release_lock(
    conn: &mut redis::aio::MultiplexedConnection,
    lock_key: &str,
    instance_id: &str,
) {
    let _: Result<i64, _> = redis::Script::new(RELEASE_LOCK_LUA)
        .key(lock_key)
        .arg(instance_id)
        .invoke_async(conn)
        .await;
}
