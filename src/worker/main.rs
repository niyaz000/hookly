use std::time::Duration;

use dotenvy::dotenv;
use tokio::task::JoinSet;
use tracing::info;

mod config;
mod consumer;
mod deliver;
mod outbox;
mod reclaim;

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

    // Ensure consumer groups exist for every stream this worker will consume.
    // Enterprise streams use "0-0" so the worker reads any messages that were
    // enqueued before this deployment; shared tier streams use "$" since prior
    // messages were handled by other workers already in the consumer group.
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

    for stream in &worker_cfg.streams {
        // Primary consumer
        set.spawn(consumer::run(
            stream.clone(),
            worker_cfg.clone(),
            db.clone(),
            redis.clone(),
            crypto.clone(),
            http.clone(),
            shutdown_rx.clone(),
        ));

        // XAUTOCLAIM reclaim task (one per stream)
        set.spawn(reclaim::run(
            stream.clone(),
            worker_cfg.clone(),
            db.clone(),
            redis.clone(),
            crypto.clone(),
            http.clone(),
            shutdown_rx.clone(),
        ));
    }

    // Outbox poller (one per worker instance, not per stream)
    set.spawn(outbox::run(
        worker_cfg.clone(),
        db.clone(),
        redis.clone(),
        shutdown_rx.clone(),
    ));

    info!(
        streams = ?worker_cfg.streams,
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
