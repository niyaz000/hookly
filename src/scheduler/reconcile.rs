use chrono::{DateTime, Utc};
use tokio::sync::watch;
use tracing::{info, warn};

use super::config::SchedulerConfig;
use super::shard::SHARDS_KEY;

/// Periodically re-syncs active schedules from PostgreSQL into their Redis sorted sets.
///
/// First run (at startup): full bootstrap — fetches all active schedules regardless
/// of when they were last modified. This covers Redis crash recovery (restart the
/// scheduler to trigger a full re-sync).
///
/// Subsequent runs: delta — only fetches schedules modified since the last reconcile,
/// so the query cost stays proportional to the change rate rather than the total count.
///
/// ZADD NX is used on sched:pending so an existing score (set by the fire loop to
/// the correct next_run_at) is never overwritten.
///
/// ZADD GT is used on sched:shards so each active shard is in the discovery set —
/// this is the safety net for the remove-while-add race and for Redis restarts.
pub async fn run(
    cfg: SchedulerConfig,
    db: sqlx::PgPool,
    redis: redis::Client,
    mut shutdown: watch::Receiver<bool>,
) {
    info!("reconcile task started");

    let mut last_reconciled_at: Option<DateTime<Utc>> = None;

    // Full bootstrap at startup.
    last_reconciled_at = Some(reconcile(&db, &redis, last_reconciled_at).await);

    let mut interval = tokio::time::interval(cfg.reconcile_interval);
    interval.tick().await; // consume the immediate first tick

    loop {
        tokio::select! {
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    info!("reconcile task shutting down");
                    return;
                }
            }
            _ = interval.tick() => {
                last_reconciled_at = Some(reconcile(&db, &redis, last_reconciled_at).await);
            }
        }
    }
}

/// Reconcile active schedules into Redis sorted sets.
///
/// Returns the timestamp at which this reconcile ran (used as the cursor for the
/// next delta query). A 1-second lookback is applied to `since` to guard against
/// sub-second clock skew between the scheduler and the database.
async fn reconcile(
    db: &sqlx::PgPool,
    redis: &redis::Client,
    since: Option<DateTime<Utc>>,
) -> DateTime<Utc> {
    let run_at = Utc::now();

    let rows: Vec<(uuid::Uuid, i16, Option<DateTime<Utc>>)> = match since {
        None => {
            // Full bootstrap: all active schedules.
            sqlx::query_as(
                r#"SELECT id, assigned_shard, next_run_at
                   FROM schedules
                   WHERE status = 'active' AND deleted_at IS NULL AND next_run_at IS NOT NULL"#,
            )
            .fetch_all(db)
            .await
        }
        Some(ts) => {
            // Delta: only schedules modified after the last reconcile.
            // 1-second lookback absorbs minor clock skew.
            let cutoff = ts - chrono::Duration::seconds(1);
            sqlx::query_as(
                r#"SELECT id, assigned_shard, next_run_at
                   FROM schedules
                   WHERE status = 'active'
                     AND deleted_at IS NULL
                     AND next_run_at IS NOT NULL
                     AND updated_at > $1"#,
            )
            .bind(cutoff)
            .fetch_all(db)
            .await
        }
    }
    .unwrap_or_else(|e| {
        warn!(error = ?e, "reconcile DB query failed");
        vec![]
    });

    let count = rows.len();

    let Ok(mut conn) = redis.get_multiplexed_async_connection().await else {
        warn!("reconcile: redis unavailable");
        return run_at;
    };

    // Collect unique shards seen in this batch so we only write to sched:shards once per shard.
    let mut active_shards = std::collections::HashSet::new();

    for (schedule_id, shard, next_run_at) in rows {
        let Some(next) = next_run_at else { continue };
        let pending_key = format!("sched:pending:{shard}");
        let score = next.timestamp() as f64;

        let _: Result<(), _> = redis::cmd("ZADD")
            .arg(&pending_key)
            .arg("NX")
            .arg(score)
            .arg(schedule_id.to_string())
            .query_async(&mut conn)
            .await;

        active_shards.insert(shard);
    }

    // Ensure every active shard is in the discovery set.
    // ZADD GT: only updates the score if the new value is higher, so this never
    // downgrades a score that the API set more recently.
    let now_ms = run_at.timestamp_millis() as f64;
    for shard in &active_shards {
        let _: Result<(), _> = redis::cmd("ZADD")
            .arg(SHARDS_KEY)
            .arg("GT")
            .arg(now_ms)
            .arg(shard.to_string())
            .query_async(&mut conn)
            .await;
    }

    let mode = if since.is_none() { "full" } else { "delta" };
    info!(count, mode, shards = active_shards.len(), "reconciled schedules into Redis");

    run_at
}
