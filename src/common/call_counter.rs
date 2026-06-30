use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};

tokio::task_local! {
    static DB_COUNT: Arc<AtomicU32>;
    static REDIS_COUNT: Arc<AtomicU32>;
}

/// Increments the per-request DB call counter. No-op when called outside a request scope.
pub fn inc_db() {
    DB_COUNT
        .try_with(|c| {
            c.fetch_add(1, Ordering::Relaxed);
        })
        .ok();
}

/// Increments the per-request Redis call counter. No-op when called outside a request scope.
pub fn inc_redis() {
    REDIS_COUNT
        .try_with(|c| {
            c.fetch_add(1, Ordering::Relaxed);
        })
        .ok();
}

/// Returns (db_calls, redis_calls) for the current request. Returns (0, 0) outside a scope.
pub fn counts() -> (u32, u32) {
    let db = DB_COUNT
        .try_with(|c| c.load(Ordering::Relaxed))
        .unwrap_or(0);
    let redis = REDIS_COUNT
        .try_with(|c| c.load(Ordering::Relaxed))
        .unwrap_or(0);
    (db, redis)
}

/// Wraps an async future in a fresh per-request counter scope.
/// Called once per request in the `set_request_id` middleware.
pub async fn scoped<F>(fut: F) -> F::Output
where
    F: std::future::Future,
{
    let db = Arc::new(AtomicU32::new(0));
    let redis = Arc::new(AtomicU32::new(0));
    DB_COUNT
        .scope(db, REDIS_COUNT.scope(redis, fut))
        .await
}
