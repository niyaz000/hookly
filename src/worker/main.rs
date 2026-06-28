use std::sync::Arc;
use std::time::Duration;

use dotenvy::dotenv;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::info;

mod circuitbreaker;
mod config;
mod consumer;
mod deliver;
mod outbox;
mod ratelimit;
mod reclaim;
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

    // Ensure consumer groups exist for all static tier streams.
    for stream in hookly::queue::TIER_STREAMS {
        let start_id = if stream.contains(":org:") { "0-0" } else { "$" };
        hookly::queue::ensure_consumer_group(&redis, stream, start_id)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("ensure_consumer_group failed for {stream}: {e}");
            });
    }

    // Register static tier streams into the scheduling sorted set.
    for stream in hookly::queue::TIER_STREAMS {
        hookly::queue::register_stream(&redis, stream)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("register_stream failed for {stream}: {e}");
            });
    }

    // One-time scan for existing enterprise streams (e.g. messages enqueued
    // before this worker pod started). Publishers keep the set current at
    // runtime, so no periodic watcher is needed.
    let enterprise_streams = hookly::queue::scan_streams(&redis, "hookly:q:org:*").await;
    for stream in &enterprise_streams {
        hookly::queue::ensure_consumer_group(&redis, stream, "0-0")
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("ensure_consumer_group (scan) failed for {stream}: {e}");
            });
        hookly::queue::register_stream(&redis, stream)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("register_stream (scan) failed for {stream}: {e}");
            });
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let sem = Arc::new(Semaphore::new(worker_cfg.max_inflight));
    let mut set = JoinSet::new();

    for worker_id in 0..worker_cfg.num_workers {
        set.spawn(consumer::run(
            worker_id,
            worker_cfg.clone(),
            db.clone(),
            redis.clone(),
            crypto.clone(),
            http.clone(),
            Arc::clone(&sem),
            shutdown_rx.clone(),
        ));
    }

    set.spawn(reclaim::run(
        worker_cfg.clone(),
        db.clone(),
        redis.clone(),
        crypto.clone(),
        http.clone(),
        shutdown_rx.clone(),
    ));

    set.spawn(trim::run(
        redis.clone(),
        worker_cfg.trim_interval_secs,
        shutdown_rx.clone(),
    ));

    set.spawn(outbox::run(
        worker_cfg.clone(),
        db.clone(),
        redis.clone(),
        shutdown_rx.clone(),
    ));

    info!(
        workers = worker_cfg.num_workers,
        consumer = %worker_cfg.consumer_name,
        enterprise_streams = enterprise_streams.len(),
        "worker started"
    );

    tokio::select! {
        _ = tokio::signal::ctrl_c() => { info!("received SIGINT, shutting down"); }
        _ = sigterm() => { info!("received SIGTERM, shutting down"); }
    }

    shutdown_tx.send(true).ok();

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
