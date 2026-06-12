use chrono::Utc;
use sqlx::PgPool;
use tokio::sync::watch;
use tracing::{error, info, warn};

use hookly::common::TenantCrypto;
use hookly::features::delivery::repository::DeliveryRepository;
use hookly::queue;

use crate::deliver;

/// Runs a blocking XREADGROUP loop for a single stream until shutdown.
///
/// Messages are processed one batch at a time. The BLOCK timeout (5 s by
/// default) lets the loop wake up periodically to check the shutdown flag.
pub async fn run(
    stream: String,
    config: crate::config::WorkerConfig,
    db: PgPool,
    redis: redis::Client,
    crypto: TenantCrypto,
    http: reqwest::Client,
    shutdown_rx: watch::Receiver<bool>,
) {
    let delivery_repo = DeliveryRepository::new(db);
    info!(stream = %stream, consumer = %config.consumer_name, "consumer started");

    loop {
        if *shutdown_rx.borrow() {
            break;
        }

        let messages = xreadgroup(&redis, &stream, &config).await;

        for (msg_id, job_pub_id) in messages {
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

        // Yield briefly so the shutdown check can fire without waiting a full block period.
        if *shutdown_rx.borrow() {
            break;
        }
    }

    info!(stream = %stream, consumer = %config.consumer_name, "consumer stopped");
}

/// Reads up to `batch_size` messages, blocking for at most `block_ms`.
async fn xreadgroup(
    redis: &redis::Client,
    stream: &str,
    config: &crate::config::WorkerConfig,
) -> Vec<(String, String)> {
    let mut conn = match redis.get_multiplexed_async_connection().await {
        Ok(c) => c,
        Err(e) => {
            warn!("xreadgroup: failed to connect: {e}");
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            return vec![];
        }
    };

    let val: redis::RedisResult<redis::Value> = redis::cmd("XREADGROUP")
        .arg("GROUP")
        .arg(queue::GROUP)
        .arg(&config.consumer_name)
        .arg("COUNT")
        .arg(config.batch_size)
        .arg("BLOCK")
        .arg(config.block_ms)
        .arg("STREAMS")
        .arg(stream)
        .arg(">") // only new, undelivered messages
        .query_async(&mut conn)
        .await;

    match val {
        Ok(v) => queue::parse_xread_reply(v),
        Err(e) => {
            warn!("xreadgroup error on {stream}: {e}");
            vec![]
        }
    }
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
        // attempt is the 0-based index of the attempt just made.
        // next_attempt is what it will be after incrementing.
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
