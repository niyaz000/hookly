use std::time::Duration;

use dotenvy::dotenv;
use tokio::task::JoinSet;
use tracing::info;
use uuid::Uuid;

mod config;
mod fire;
mod reconcile;
mod shard;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let cfg = hookly::config::Config::from_env().expect("Failed to load configuration");
    let _otel = hookly::telemetry::init(&cfg);

    let scheduler_cfg = config::SchedulerConfig::default();

    let db = sqlx::PgPool::connect(&cfg.database.url)
        .await
        .expect("Failed to connect to database");

    let redis = redis::Client::open(cfg.redis.url.as_str()).expect("Invalid Redis URL");

    let instance_id = Uuid::now_v7().to_string();

    info!(
        instance_id = %instance_id,
        shard_count = scheduler_cfg.shard_count,
        "scheduler starting"
    );

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mut set = JoinSet::new();

    // One task per shard.
    for shard_id in 0..scheduler_cfg.shard_count {
        set.spawn(shard::run(
            shard_id,
            instance_id.clone(),
            scheduler_cfg.clone(),
            db.clone(),
            redis.clone(),
            shutdown_rx.clone(),
        ));
    }

    // Reconciliation task runs independently of shard ownership.
    set.spawn(reconcile::run(
        scheduler_cfg.clone(),
        db.clone(),
        redis.clone(),
        shutdown_rx.clone(),
    ));

    // Wait for shutdown signal.
    tokio::select! {
        _ = tokio::signal::ctrl_c() => { info!("received SIGINT, shutting down scheduler"); }
        _ = sigterm() => { info!("received SIGTERM, shutting down scheduler"); }
    }

    shutdown_tx.send(true).ok();

    let deadline = tokio::time::sleep(Duration::from_secs(10));
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

    info!("scheduler stopped");
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
