use std::sync::Arc;

use tokio::sync::{RwLock, watch};
use tracing::{info, warn};

/// Periodically scans Redis for streams matching `hookly:q:*` and adds any
/// newly discovered ones (e.g. a freshly created enterprise org stream) to the
/// shared list that all worker tasks read.
///
/// Workers pick up the new stream on their next XREADGROUP iteration, which
/// happens within `block_ms` (default 5 s).
pub async fn run(
    streams: Arc<RwLock<Vec<String>>>,
    redis: redis::Client,
    interval_secs: u64,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    info!("stream watcher started");
    let mut interval =
        tokio::time::interval(std::time::Duration::from_secs(interval_secs));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Skip the first tick — streams were already initialised in main().
    interval.tick().await;

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = shutdown_rx.changed() => { break; }
        }
        if *shutdown_rx.borrow() {
            break;
        }

        let discovered = hookly::queue::scan_streams(&redis, "hookly:q:*").await;
        let mut current = streams.write().await;

        for stream in discovered {
            if !current.contains(&stream) {
                // "0-0" so the worker reads any messages enqueued before it started.
                match hookly::queue::ensure_consumer_group(&redis, &stream, "0-0").await {
                    Ok(_) => {
                        info!(stream = %stream, "discovered new stream, adding to pool");
                        current.push(stream);
                    }
                    Err(e) => {
                        warn!(stream = %stream, "stream discovery: ensure_consumer_group failed: {e}");
                    }
                }
            }
        }
    }

    info!("stream watcher stopped");
}
