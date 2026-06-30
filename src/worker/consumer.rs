use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use sqlx::PgPool;
use tokio::sync::{watch, Semaphore};
use tracing::{error, info, warn};

use hookly::common::{CountingPool, TenantCrypto};
use hookly::features::delivery::repository::DeliveryRepository;
use hookly::queue;

use crate::circuitbreaker;
use crate::deliver;
use crate::ratelimit;

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
///
/// Each message in a batch is spawned as an independent Tokio task, bounded
/// by the shared `sem` so total in-flight deliveries across all workers in
/// this pod never exceed `max_inflight`. Slow endpoints don't block the
/// worker from claiming the next stream or processing other messages.
pub async fn run(
    worker_id: usize,
    config: crate::config::WorkerConfig,
    db: PgPool,
    redis: redis::Client,
    crypto: TenantCrypto,
    http: reqwest::Client,
    sem: Arc<Semaphore>,
    shutdown_rx: watch::Receiver<bool>,
) {
    let consumer_name = format!("{}-w{}", config.consumer_name, worker_id);
    let delivery_repo = DeliveryRepository::new(CountingPool::from(db));
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

            // Acquire a semaphore permit before spawning. This blocks the
            // claiming loop when max_inflight tasks are already in-flight,
            // providing backpressure without unbounded task growth.
            let permit = Arc::clone(&sem)
                .acquire_owned()
                .await
                .expect("semaphore closed");

            let repo = delivery_repo.clone();
            let cry = crypto.clone();
            let h = http.clone();
            let r = redis.clone();
            let s = stream.clone();
            let cfg = config.clone();

            tokio::spawn(async move {
                let _permit = permit; // released when task finishes
                process_one(&msg_id, &job_pub_id, &s, &repo, &cry, &h, &r, &cfg).await;
            });
        }

        if *shutdown_rx.borrow() {
            break;
        }
    }

    info!(worker_id, "consumer stopped");
}

/// Processes a single message end-to-end: fetch → guard checks → deliver → record → XACK.
///
/// Does NOT XACK only when the DB fetch fails — XAUTOCLAIM will retry after the
/// idle threshold (genuine crash recovery path).
///
/// For intentional skips (rate limited, circuit open): XACKs immediately and calls
/// defer_job so the outbox re-enqueues at the precise retry time. This keeps PEL
/// strictly for crash recovery rather than conflating it with retry scheduling.
pub async fn process_one(
    msg_id: &str,
    job_pub_id: &str,
    stream: &str,
    delivery_repo: &DeliveryRepository,
    crypto: &TenantCrypto,
    http: &reqwest::Client,
    redis: &redis::Client,
    config: &crate::config::WorkerConfig,
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

    // ── Guard checks: defer rather than leave in PEL ─────────────────────────
    //
    // All three guards XACK the message and call defer_job (no attempt increment)
    // so the outbox re-enqueues at the right time. PEL is reserved for genuine
    // crash recovery, not intentional skips with a known retry window.

    // 1. Blocked by a prior 429 response — defer until the block key expires.
    if let Some(remaining_secs) = ratelimit::blocked_remaining_secs(redis, job.endpoint_id).await {
        let retry_at = Utc::now() + chrono::Duration::seconds(remaining_secs.max(1) as i64);
        if let Err(e) = delivery_repo.defer_job(job.job_id, retry_at).await {
            error!(job_public_id = %job.job_public_id, "defer_job (blocked) failed: {e:?}");
        }
        queue::xack(redis, stream, msg_id).await.ok();
        warn!(
            job_public_id = %job.job_public_id,
            endpoint_id   = %job.endpoint_id,
            defer_secs    = remaining_secs,
            "endpoint blocked by prior 429, deferred"
        );
        return;
    }

    // 2. Circuit open — defer until cooldown expires.
    if let Some(remaining_secs) = circuitbreaker::open_remaining_secs(redis, job.endpoint_id).await {
        let retry_at = Utc::now() + chrono::Duration::seconds(remaining_secs.max(1) as i64);
        if let Err(e) = delivery_repo.defer_job(job.job_id, retry_at).await {
            error!(job_public_id = %job.job_public_id, "defer_job (circuit open) failed: {e:?}");
        }
        queue::xack(redis, stream, msg_id).await.ok();
        warn!(
            job_public_id = %job.job_public_id,
            endpoint_id   = %job.endpoint_id,
            defer_secs    = remaining_secs,
            "circuit open, deferred"
        );
        return;
    }

    // 3. Proactive per-minute rate limit — defer until the next minute bucket.
    if let Some(limit) = job.rate_limit_per_minute {
        if !ratelimit::try_acquire(redis, job.endpoint_id, limit).await {
            let now_secs = Utc::now().timestamp() as u64;
            let secs_to_next_minute = 60 - (now_secs % 60);
            let retry_at = Utc::now() + chrono::Duration::seconds(secs_to_next_minute as i64);
            if let Err(e) = delivery_repo.defer_job(job.job_id, retry_at).await {
                error!(job_public_id = %job.job_public_id, "defer_job (rate limit) failed: {e:?}");
            }
            queue::xack(redis, stream, msg_id).await.ok();
            warn!(
                job_public_id = %job.job_public_id,
                endpoint_id   = %job.endpoint_id,
                limit_per_min = limit,
                defer_secs    = secs_to_next_minute,
                "rate limit exceeded, deferred until next minute"
            );
            return;
        }
    }

    // ── Delivery ──────────────────────────────────────────────────────────────

    info!(
        job_public_id  = %job.job_public_id,
        event_public_id = %job.event_public_id,
        attempt        = job.attempt + 1,
        "delivering"
    );

    let result = deliver::deliver(&job, crypto, http).await;

    // On 429: block the endpoint so subsequent messages skip the HTTP call
    // until the window expires. The retry scheduler handles re-enqueuing.
    if result.http_status == Some(429) {
        let ttl = result.retry_after_secs.unwrap_or(60);
        ratelimit::set_blocked(redis, job.endpoint_id, ttl).await;
        warn!(
            job_public_id = %job.job_public_id,
            endpoint_id   = %job.endpoint_id,
            ttl_secs      = ttl,
            "endpoint returned 429, blocked"
        );
    }

    // Circuit breaker: count endpoint faults (5xx, timeout, network error).
    // 4xx responses are the sender's fault and do not indicate endpoint health issues.
    let is_endpoint_fault = matches!(result.status, deliver::DeliveryStatus::Timeout)
        || (matches!(result.status, deliver::DeliveryStatus::Failed)
            && result.http_status.map_or(true, |s| s >= 500));

    if is_endpoint_fault {
        let tripped = circuitbreaker::record_failure(
            redis,
            job.endpoint_id,
            config.cb_failure_threshold,
            config.cb_window_secs,
        )
        .await;
        if tripped {
            circuitbreaker::open_circuit(redis, job.endpoint_id, config.cb_cooldown_secs).await;
            warn!(
                job_public_id     = %job.job_public_id,
                endpoint_id       = %job.endpoint_id,
                cooldown_secs     = config.cb_cooldown_secs,
                "circuit opened after repeated endpoint failures"
            );
        }
    } else if result.status.is_success() {
        circuitbreaker::reset(redis, job.endpoint_id).await;
    }

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
                attempt       = next_attempt,
                max_attempts  = job.max_attempts,
                retry_after   = %retry_after,
                "delivery failed, scheduled for retry"
            );
            if let Err(e) = delivery_repo.schedule_retry(job.job_id, retry_after).await {
                error!(job_public_id = %job.job_public_id, "schedule_retry failed: {e:?}");
            }
        } else {
            warn!(
                job_public_id = %job.job_public_id,
                attempt       = next_attempt,
                max_attempts  = job.max_attempts,
                "delivery failed, max attempts reached"
            );
            if let Err(e) = delivery_repo.fail_job(job.job_id).await {
                error!(job_public_id = %job.job_public_id, "fail_job failed: {e:?}");
            }
        }
    }

    queue::xack(redis, stream, msg_id).await.ok();
}
