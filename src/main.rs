use std::time::Duration;

use dotenvy::dotenv;
use hookly::queue;

mod common;
mod config;
mod email;
mod error;
mod features;
mod router;
mod state;
mod telemetry;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let config = config::Config::from_env().expect("Failed to load configuration");
    let _otel = telemetry::init(&config);
    let state = state::AppState::new(&config)
        .await
        .expect("Failed to initialize AppState");

    for stream in queue::TIER_STREAMS {
        queue::ensure_consumer_group(&state.redis, stream, "$")
            .await
            .expect("Failed to ensure Redis consumer group");
    }

    // Background task: disable JWT keys whose rotation grace period has ended
    {
        use features::jwt_keys::repository::JwtKeyRepository;
        let repo = JwtKeyRepository::new(state.db.clone());
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3600));
            loop {
                interval.tick().await;
                match repo.expire_grace_period_keys().await {
                    Ok(n) if n > 0 => tracing::info!(count = n, "expired rotated jwt keys"),
                    Err(e) => tracing::warn!(error = ?e, "failed to expire rotated jwt keys"),
                    _ => {}
                }
            }
        });
    }

    let app = router::create_router(state);

    let addr = config.server.addr();
    tracing::info!(addr = %addr, "server starting");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
