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

async fn create_application(app: axum::Router) -> Value {
    let body = json!({
        "tenant_id": Uuid::new_v4(),
        "organization_id": Uuid::new_v4(),
        "name": "get-test-app",
        "description": "for get endpoint tests",
        "tags": { "env": "test" }
    });

    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/applications")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn test_get_application_success(pool: PgPool) {
    let app = create_router(test_state(pool));
    let created = create_application(app.clone()).await;
    let public_id = created["data"]["public_id"].as_str().unwrap();

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/v1/applications/{public_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["public_id"], public_id);
    assert_eq!(json["data"]["name"], "get-test-app");
    assert_eq!(json["data"]["description"], "for get endpoint tests");
    assert_eq!(json["data"]["tags"]["env"], "test");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_get_application_not_found(pool: PgPool) {
    let app = create_router(test_state(pool));

    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/applications/app_doesnotexist")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(
        json["error"],
        "Application not found: app_doesnotexist"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn test_delete_application_success(pool: PgPool) {
    let app = create_router(test_state(pool));
    let created = create_application(app.clone()).await;
    let public_id = created["data"]["public_id"].as_str().unwrap();

    let request = Request::builder()
        .method("DELETE")
        .uri(format!("/api/v1/applications/{public_id}"))
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_delete_application_is_noop_when_already_deleted(pool: PgPool) {
    let app = create_router(test_state(pool));
    let created = create_application(app.clone()).await;
    let public_id = created["data"]["public_id"].as_str().unwrap();

    let delete = |app: axum::Router| {
        let uri = format!("/api/v1/applications/{public_id}");
        async move {
            let request = Request::builder()
                .method("DELETE")
                .uri(uri)
                .body(Body::empty())
                .unwrap();
            app.oneshot(request).await.unwrap()
        }
    };

    let first = delete(app.clone()).await;
    assert_eq!(first.status(), StatusCode::NO_CONTENT);

    let second = delete(app.clone()).await;
    assert_eq!(second.status(), StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_delete_application_not_found_is_noop(pool: PgPool) {
    let app = create_router(test_state(pool));

    let request = Request::builder()
        .method("DELETE")
        .uri("/api/v1/applications/app_doesnotexist")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}
