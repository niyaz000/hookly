use std::sync::Arc;

use tokio::sync::{RwLock, watch};
use tracing::info;

/// Periodically trims each stream using a data-safe MINID strategy:
///
/// - PEL non-empty → XTRIM MINID = oldest pending entry.
///   Everything before it is guaranteed ACK'd (Redis delivers in monotonic order).
/// - PEL empty → XTRIM MINID = last-delivered-id from XINFO GROUPS.
///   All entries were consumed and ACK'd; stream is safe to compact.
///
/// This prevents memory growth without risking loss of undelivered messages.
/// Contrast with MAXLEN, which can trim entries that are in the stream but not
/// yet consumed — those have `enqueued_at IS NOT NULL` so the outbox poller
/// would never recover them.
pub async fn run(
    streams: Arc<RwLock<Vec<String>>>,
    redis: redis::Client,
    interval_secs: u64,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    info!("stream trimmer started");
    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(interval_secs));
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

        let current = streams.read().await.clone();
        for stream in &current {
            hookly::queue::xtrim_safe(&redis, stream).await;
        }
    }

    info!("stream trimmer stopped");
}
