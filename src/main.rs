use dotenvy::dotenv;
use tracing_subscriber;

mod config;
mod state;
mod error;
mod router;
mod common;
mod features;

#[tokio::main]
async fn main() {
    dotenv().ok();
    
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = config::Config::from_env().expect("Failed to load configuration");
    let state = state::AppState::new(&config).await.expect("Failed to initialize AppState");

    let app = router::create_router(state);

    let addr = config.server.addr();
    println!("🚀 Server running on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
