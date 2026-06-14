use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use sqlx::PgPool;
use tokio::sync::watch;
use tracing::{error, info, warn};

use hookly::common::TenantCrypto;
use hookly::features::delivery::repository::DeliveryRepository;
use hookly::queue;

use crate::deliver;

/// Runs a single worker task using sorted-set-based stream scheduling.
///
/// Each iteration atomically claims the stream with the lowest score (least
/// recently consumed) by updating its score to now_ms. This is the only
/// coordination needed: Redis single-threaded Lua execution guarantees no two
/// workers claim the same stream simultaneously.
///
/// When a stream is empty it is removed from the set (atomically, to avoid a
/// race with concurrent publishers). Publishers re-register a stream via
/// register_stream(NX) on every enqueue, so empty removal is safe.
///
/// Workers sleep only when the sorted set is empty (all streams drained).
/// Once any new event arrives the publisher re-adds the stream and workers
/// pick it up within poll_interval_ms.
pub async fn run(
    worker_id: usize,
    config: crate::config::WorkerConfig,
    db: PgPool,
    redis: redis::Client,
    crypto: TenantCrypto,
    http: reqwest::Client,
    shutdown_rx: watch::Receiver<bool>,
) {
    let consumer_name = format!("{}-w{}", config.consumer_name, worker_id);
    let delivery_repo = DeliveryRepository::new(db);
    info!(worker_id, consumer = %consumer_name, "consumer started");

    loop {
        if *shutdown_rx.borrow() {
            break;
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let stream = match queue::claim_next_stream(&redis, now_ms).await {
            Some(s) => s,
            None => {
                // No streams registered — all are drained or none yet exist.
                tokio::time::sleep(Duration::from_millis(config.poll_interval_ms)).await;
                continue;
            }
        };

        let messages =
            queue::xreadgroup_single(&redis, &stream, &consumer_name, config.batch_size).await;

        if messages.is_empty() {
            queue::remove_stream_if_empty(&redis, &stream).await;
            continue;
        }

        for (msg_id, job_pub_id) in messages {
            if *shutdown_rx.borrow() {
                break;
            }
            process_one(
                &msg_id,
                &job_pub_id,
                &stream,
                &delivery_repo,
                &crypto,
                &http,
                &redis,
            )
            .await;
        }

        if *shutdown_rx.borrow() {
            break;
        }
    }

    info!(worker_id, "consumer stopped");
}

/// Processes a single message end-to-end: fetch → deliver → record → XACK.
///
/// Always XACKs, even on error. The delivery_attempt row captures the outcome.
pub async fn process_one(
    msg_id: &str,
    job_pub_id: &str,
    stream: &str,
    delivery_repo: &DeliveryRepository,
    crypto: &TenantCrypto,
    http: &reqwest::Client,
    redis: &redis::Client,
) {
    let job = match delivery_repo.get_job_for_delivery(job_pub_id).await {
        Ok(Some(j)) => j,
        Ok(None) => {
            // Already delivered or endpoint inactive — nothing to do.
            queue::xack(redis, stream, msg_id).await.ok();
            return;
        }
        Err(e) => {
            error!(job_pub_id, "DB fetch failed: {e:?}");
            // Don't XACK — XAUTOCLAIM will retry after the idle threshold.
            return;
        }
    };

    info!(
        job_public_id = %job.job_public_id,
        event_public_id = %job.event_public_id,
        attempt = job.attempt + 1,
        "delivering"
    );

    let result = deliver::deliver(&job, crypto, http).await;

    if let Err(e) = delivery_repo
        .insert_attempt(
            job.job_id,
            job.event_id,
            job.endpoint_id,
            job.attempt + 1,
            result.status.as_str(),
            result.http_status,
            result.response_body.as_deref(),
            Some(result.latency_ms),
        )
        .await
    {
        error!(job_public_id = %job.job_public_id, "insert_attempt failed: {e:?}");
    }

    if result.status.is_success() {
        if let Err(e) = delivery_repo.complete_job(job.job_id).await {
            error!(job_public_id = %job.job_public_id, "complete_job failed: {e:?}");
        }
        info!(job_public_id = %job.job_public_id, "delivered successfully");
    } else {
        let next_attempt = job.attempt + 1;
        if next_attempt < job.max_attempts {
            let backoff_secs = (30u64 * 2u64.pow(job.attempt as u32)).min(3600);
            let retry_after = Utc::now() + chrono::Duration::seconds(backoff_secs as i64);
            warn!(
                job_public_id = %job.job_public_id,
                attempt = next_attempt,
                max_attempts = job.max_attempts,
                retry_after = %retry_after,
                "delivery failed, scheduled for retry"
            );
            if let Err(e) = delivery_repo.schedule_retry(job.job_id, retry_after).await {
                error!(job_public_id = %job.job_public_id, "schedule_retry failed: {e:?}");
            }
        } else {
            warn!(
                job_public_id = %job.job_public_id,
                attempt = next_attempt,
                max_attempts = job.max_attempts,
                "delivery failed, max attempts reached"
            );
            if let Err(e) = delivery_repo.fail_job(job.job_id).await {
                error!(job_public_id = %job.job_public_id, "fail_job failed: {e:?}");
            }
        }
    }

    queue::xack(redis, stream, msg_id).await.ok();
}
