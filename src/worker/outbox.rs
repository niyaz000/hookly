use tokio::sync::watch;
use tracing::{info, warn};

use hookly::features::delivery::repository::DeliveryRepository;
use hookly::queue;

/// Periodically finds delivery jobs that never made it into Redis (enqueued_at
/// IS NULL) and re-enqueues them. This is the safety net for XADD failures.
///
/// At-least-once semantics: if a job is picked up by two poller instances
/// simultaneously, it may be enqueued twice. Workers handle this gracefully
/// because `get_job_for_delivery` checks `status IN ('pending', 'retrying')` —
/// a job that's already been delivered returns None and the duplicate XACK is a no-op.
pub async fn run(
    config: crate::config::WorkerConfig,
    db: sqlx::PgPool,
    redis: redis::Client,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let delivery_repo = DeliveryRepository::new(db);
    let mut interval = tokio::time::interval(config.outbox_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Skip the first immediate tick so the consumer has a chance to start first.
    interval.tick().await;

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = shutdown_rx.changed() => { break; }
        }
        if *shutdown_rx.borrow() {
            break;
        }

        let jobs = match delivery_repo.list_unqueued(500).await {
            Ok(j) => j,
            Err(e) => {
                warn!("outbox: list_unqueued failed: {e:?}");
                continue;
            }
        };

        if jobs.is_empty() {
            continue;
        }

        info!(
            count = jobs.len(),
            "outbox: re-enqueueing missed delivery jobs"
        );

        for job in jobs {
            match queue::enqueue(&redis, &job.stream_name, &job.public_id).await {
                Ok(_) => {
                    if let Err(e) = queue::register_stream(&redis, &job.stream_name).await {
                        warn!(stream = %job.stream_name, "outbox: register_stream failed: {e}");
                    }
                    if let Err(e) = delivery_repo.mark_enqueued(job.id).await {
                        warn!(job_public_id = %job.public_id, "outbox: mark_enqueued failed: {e:?}");
                    }
                }
                Err(e) => {
                    warn!(job_public_id = %job.public_id, "outbox: XADD failed: {e}");
                }
            }
        }
    }
}
