use dotenvy::dotenv;
use hookly::queue;

mod common;
mod config;
mod email;
mod error;
mod features;
mod router;
mod state;

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::fmt()
        .json()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = config::Config::from_env().expect("Failed to load configuration");
    let state = state::AppState::new(&config)
        .await
        .expect("Failed to initialize AppState");

    for stream in queue::TIER_STREAMS {
        queue::ensure_consumer_group(&state.redis, stream, "$")
            .await
            .expect("Failed to ensure Redis consumer group");
    }

    let app = router::create_router(state);

    let addr = config.server.addr();
    tracing::info!(addr = %addr, "server starting");

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
