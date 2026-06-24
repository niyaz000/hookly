use tokio::sync::watch;
use tracing::info;

/// Periodically trims each active stream using a data-safe MINID strategy.
/// Reads the current stream list from the scheduling sorted set so it stays
/// in sync with the worker's view of active streams without a separate shared Vec.
pub async fn run(redis: redis::Client, interval_secs: u64, mut shutdown_rx: watch::Receiver<bool>) {
    info!("stream trimmer started");
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    interval.tick().await;

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = shutdown_rx.changed() => { break; }
        }
        if *shutdown_rx.borrow() {
            break;
        }

        let current = hookly::queue::list_scheduled_streams(&redis).await;
        for stream in &current {
            hookly::queue::xtrim_safe(&redis, stream).await;
        }
    }

    info!("stream trimmer stopped");
}
