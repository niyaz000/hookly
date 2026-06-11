use redis::AsyncCommands;
use tokio::sync::watch;
use tracing::{info, warn};

use super::config::SchedulerConfig;

/// Periodically re-syncs all active schedules from PostgreSQL into their Redis
/// sorted sets. This repairs drift caused by scheduler downtime, Redis crashes,
/// or missed ZADD calls.
///
/// Uses NX so it never overwrites a score that was correctly set by the fire loop.
pub async fn run(
    cfg: SchedulerConfig,
    db: sqlx::PgPool,
    redis: redis::Client,
    mut shutdown: watch::Receiver<bool>,
) {
    info!("reconcile task started");

    // Run once at startup to bootstrap the sorted sets, then on the interval.
    reconcile(&db, &redis).await;

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
                reconcile(&db, &redis).await;
            }
        }
    }
}

async fn reconcile(db: &sqlx::PgPool, redis: &redis::Client) {
    let rows: Vec<(uuid::Uuid, i16, Option<chrono::DateTime<chrono::Utc>>)> = match sqlx::query_as(
        r#"SELECT id, assigned_shard, next_run_at
           FROM schedules
           WHERE status = 'active' AND deleted_at IS NULL AND next_run_at IS NOT NULL"#,
    )
    .fetch_all(db)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!(error = ?e, "reconcile DB query failed");
            return;
        }
    };

    let count = rows.len();

    let Ok(mut conn) = redis.get_multiplexed_async_connection().await else {
        warn!("reconcile: redis unavailable");
        return;
    };

    for (schedule_id, shard, next_run_at) in rows {
        let Some(next) = next_run_at else { continue };
        let pending_key = format!("sched:pending:{shard}");
        let score = next.timestamp() as f64;

        // NX: only add if the member is absent. The fire loop sets the correct
        // score after each fire; we don't want to overwrite that.
        let _: Result<(), _> = redis::cmd("ZADD")
            .arg(&pending_key)
            .arg("NX")
            .arg(score)
            .arg(schedule_id.to_string())
            .query_async(&mut conn)
            .await;
    }

    info!(count = count, "reconciled schedules into Redis sorted sets");
}
