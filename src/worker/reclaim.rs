use sqlx::PgPool;
use tokio::sync::watch;
use tracing::info;

use hookly::common::TenantCrypto;
use hookly::features::delivery::repository::DeliveryRepository;
use hookly::queue;

use crate::consumer;

/// Periodically runs XAUTOCLAIM across all active streams to recover messages
/// that were assigned to a crashed or slow consumer and have been idle beyond
/// `reclaim_idle_ms`. Reclaimed messages are processed immediately.
pub async fn run(
    config: crate::config::WorkerConfig,
    db: PgPool,
    redis: redis::Client,
    crypto: TenantCrypto,
    http: reqwest::Client,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let delivery_repo = DeliveryRepository::new(db);
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Skip the first immediate tick.
    interval.tick().await;

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = shutdown_rx.changed() => { break; }
        }
        if *shutdown_rx.borrow() {
            break;
        }

        let current_streams = queue::list_scheduled_streams(&redis).await;
        for stream in &current_streams {
            let claimed = queue::xautoclaim(
                &redis,
                stream,
                &config.consumer_name,
                config.reclaim_idle_ms,
            )
            .await;

            if !claimed.is_empty() {
                info!(stream = %stream, count = claimed.len(), "reclaiming idle messages");
            }

            for (msg_id, job_pub_id) in claimed {
                consumer::process_one(
                    &msg_id,
                    &job_pub_id,
                    stream,
                    &delivery_repo,
                    &crypto,
                    &http,
                    &redis,
                    &config,
                )
                .await;
            }
        }
    }
}
