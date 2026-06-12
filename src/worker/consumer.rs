use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sqlx::PgPool;
use tokio::sync::{RwLock, watch};
use tracing::{error, info, warn};

use hookly::common::TenantCrypto;
use hookly::features::delivery::repository::DeliveryRepository;
use hookly::queue;

use crate::deliver;

/// Runs a single worker task with fair round-robin scheduling across streams.
///
/// Each iteration reads one stream non-blocking and then advances to the next.
/// A stream with a large backlog never starves other streams: every stream gets
/// one turn per rotation regardless of how many messages it holds.
///
/// Sleep behaviour: workers only pause when a full rotation (all streams)
/// returns empty, keeping latency low under load while avoiding a busy-loop at
/// idle. `WORKER_POLL_INTERVAL_MS` (default 250 ms) controls the idle sleep.
///
/// Staggered start: `worker_id` sets the initial stream index so N workers
/// spread their first reads across N different streams.
pub async fn run(
    worker_id: usize,
    streams: Arc<RwLock<Vec<String>>>,
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

    // Each worker starts at a different stream to spread the first-read load.
    let mut stream_idx = worker_id;
    // Counts consecutive empty reads. Once it reaches the number of streams in
    // a rotation, we know the whole list was empty and sleep before retrying.
    let mut empty_streak: usize = 0;

    loop {
        if *shutdown_rx.borrow() {
            break;
        }

        let current = streams.read().await.clone();
        let n = current.len();

        if n == 0 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            continue;
        }

        // Wrap index safely when the streams list grows or shrinks.
        stream_idx %= n;
        let stream = current[stream_idx].clone();

        // Always advance before processing so a panic in process_one doesn't
        // pin the worker on the same stream indefinitely.
        stream_idx = (stream_idx + 1) % n;

        let messages = queue::xreadgroup_single(
            &redis,
            &stream,
            &consumer_name,
            config.batch_size,
        )
        .await;

        if messages.is_empty() {
            empty_streak += 1;
            // After one full rotation with no messages, sleep to avoid hammering
            // Redis and wasting CPU. Reset the streak after sleeping.
            if empty_streak >= n {
                tokio::time::sleep(Duration::from_millis(config.poll_interval_ms)).await;
                empty_streak = 0;
                if *shutdown_rx.borrow() {
                    break;
                }
            }
            continue;
        }

        // Got messages from this stream — reset the idle counter and process.
        empty_streak = 0;

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
            // Exponential backoff: 30s, 60s, 120s, 240s, … capped at 1 hour.
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
