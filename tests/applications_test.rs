use axum::{body::Body, http::{Request, StatusCode}};
use redis::Client as RedisClient;
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use hookly::{router::create_router, state::AppState};

fn test_state(pool: PgPool) -> AppState {
    let redis = RedisClient::open("redis://127.0.0.1/").expect("Redis URL invalid");
    AppState { db: pool, redis }
}

#[sqlx::test(migrations = "./migrations")]
async fn test_create_application_success(pool: PgPool) {
    let app = create_router(test_state(pool));

    let body = json!({
        "tenant_id": Uuid::new_v4(),
        "organization_id": Uuid::new_v4(),
        "name": "my-test-app",
        "description": "integration test application",
        "tags": { "env": "test" }
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/applications")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["success"], true);

    let data = &json["data"];
    assert!(!data["id"].as_str().unwrap().is_empty());
    assert!(!data["public_id"].as_str().unwrap().is_empty());
    assert_eq!(data["name"], "my-test-app");
    assert_eq!(data["description"], "integration test application");
    assert_eq!(data["tags"]["env"], "test");
    assert!(data["created_at"].is_string());
    assert!(data["updated_at"].is_string());
}
