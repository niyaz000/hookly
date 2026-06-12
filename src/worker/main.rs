use std::sync::Arc;
use std::time::Duration;

use dotenvy::dotenv;
use tokio::sync::RwLock;
use tokio::task::JoinSet;
use tracing::info;

mod config;
mod consumer;
mod deliver;
mod outbox;
mod reclaim;
mod stream_watcher;
mod trim;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let cfg = hookly::config::Config::from_env().expect("Failed to load configuration");
    let _otel = hookly::telemetry::init(&cfg);
    let worker_cfg = config::WorkerConfig::from_env();

    let db = sqlx::PgPool::connect(&cfg.database.url)
        .await
        .expect("Failed to connect to database");

    let redis = redis::Client::open(cfg.redis.url.as_str()).expect("Invalid Redis URL");
    let crypto = hookly::common::TenantCrypto::new(&cfg.crypto.master_key)
        .expect("Invalid CRYPTO_MASTER_KEY");

    let http = reqwest::Client::builder()
        .timeout(worker_cfg.delivery_timeout)
        .build()
        .expect("Failed to create HTTP client");

    // Shared, mutable stream list. Workers, reclaim, and watcher all hold a
    // clone of this Arc. The stream-watcher appends new streams; workers read
    // it at the top of each XREADGROUP iteration.
    let streams: Arc<RwLock<Vec<String>>> = Arc::new(RwLock::new(worker_cfg.streams.clone()));

    // Ensure consumer groups exist for the initial set of streams.
    // Enterprise streams use "0-0" so a newly-deployed worker catches up on
    // any messages that were enqueued before it started.
    for stream in &worker_cfg.streams {
        let start_id = if stream.contains(":org:") { "0-0" } else { "$" };
        hookly::queue::ensure_consumer_group(&redis, stream, start_id)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("ensure_consumer_group failed for {stream}: {e}");
            });
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mut set = JoinSet::new();

    // N concurrent workers, each consuming from ALL streams in one XREADGROUP
    // call. Redis load-balances messages across them automatically.
    for worker_id in 0..worker_cfg.num_workers {
        set.spawn(consumer::run(
            worker_id,
            Arc::clone(&streams),
            worker_cfg.clone(),
            db.clone(),
            redis.clone(),
            crypto.clone(),
            http.clone(),
            shutdown_rx.clone(),
        ));
    }

    // One reclaim task iterates over the full shared stream list each tick.
    set.spawn(reclaim::run(
        Arc::clone(&streams),
        worker_cfg.clone(),
        db.clone(),
        redis.clone(),
        crypto.clone(),
        http.clone(),
        shutdown_rx.clone(),
    ));

    // Auto-discovery: scans Redis every WORKER_STREAM_WATCH_INTERVAL_SECS and
    // adds any new streams (e.g. a new enterprise org) to the shared list.
    set.spawn(stream_watcher::run(
        Arc::clone(&streams),
        redis.clone(),
        worker_cfg.stream_watch_interval_secs,
        shutdown_rx.clone(),
    ));

    // Safe trimming: XTRIM MINID on each stream every WORKER_TRIM_INTERVAL_SECS.
    set.spawn(trim::run(
        Arc::clone(&streams),
        redis.clone(),
        worker_cfg.trim_interval_secs,
        shutdown_rx.clone(),
    ));

    // Outbox poller: re-enqueues jobs where XADD failed (enqueued_at IS NULL)
    // and retrying jobs whose retry_after has passed.
    set.spawn(outbox::run(
        worker_cfg.clone(),
        db.clone(),
        redis.clone(),
        shutdown_rx.clone(),
    ));

    info!(
        streams = ?worker_cfg.streams,
        workers = worker_cfg.num_workers,
        consumer = %worker_cfg.consumer_name,
        "worker started"
    );

    // Wait for shutdown signal.
    tokio::select! {
        _ = tokio::signal::ctrl_c() => { info!("received SIGINT, shutting down"); }
        _ = sigterm() => { info!("received SIGTERM, shutting down"); }
    }

    shutdown_tx.send(true).ok();

    // Give tasks up to 15 s to finish in-flight deliveries.
    let deadline = tokio::time::sleep(Duration::from_secs(15));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            result = set.join_next() => {
                if result.is_none() { break; }
            }
            _ = &mut deadline => {
                tracing::warn!("shutdown deadline reached, aborting remaining tasks");
                set.abort_all();
                break;
            }
        }
    }

    info!("worker stopped");
}

#[cfg(unix)]
async fn sigterm() {
    use tokio::signal::unix::{signal, SignalKind};
    signal(SignalKind::terminate())
        .expect("failed to install SIGTERM handler")
        .recv()
        .await;
}

#[cfg(not(unix))]
async fn sigterm() {
    std::future::pending::<()>().await;
}
