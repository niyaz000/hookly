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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn post_json(app: axum::Router, uri: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn send(app: axum::Router, method: &str, uri: &str) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, json)
}

async fn create_test_application(app: axum::Router) -> Value {
    let body = json!({
        "tenant_id": Uuid::new_v4(),
        "organization_id": Uuid::new_v4(),
        "name": format!("test-app-{}", Uuid::new_v4()),
        "description": "test application",
        "tags": { "env": "test" }
    });
    let (status, json) = post_json(app, "/api/v1/applications", body).await;
    assert_eq!(status, StatusCode::CREATED);
    json
}

fn assert_application_fields(data: &Value) {
    assert!(!data["id"].as_str().unwrap_or("").is_empty());
    assert!(!data["public_id"].as_str().unwrap_or("").is_empty());
    assert!(!data["created_by"].as_str().unwrap_or("").is_empty());
    assert!(!data["updated_by"].as_str().unwrap_or("").is_empty());
    assert!(data["created_at"].is_string());
    assert!(data["updated_at"].is_string());
    assert!(data["state"].is_string());
}

// ---------------------------------------------------------------------------
// Create
// ---------------------------------------------------------------------------

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
    let (status, json) = post_json(app, "/api/v1/applications", body).await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(json["success"], true);

    let data = &json["data"];
    assert_application_fields(data);
    assert_eq!(data["name"], "my-test-app");
    assert_eq!(data["description"], "integration test application");
    assert_eq!(data["tags"]["env"], "test");
    assert_eq!(data["state"], "Active");
}

// ---------------------------------------------------------------------------
// Get
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn test_get_application_success(pool: PgPool) {
    let app = create_router(test_state(pool));
    let created = create_test_application(app.clone()).await;
    let public_id = created["data"]["public_id"].as_str().unwrap();

    let (status, json) = send(app, "GET", &format!("/api/v1/applications/{public_id}")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    let data = &json["data"];
    assert_application_fields(data);
    assert_eq!(data["public_id"], public_id);
    assert_eq!(data["state"], "Active");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_get_application_not_found(pool: PgPool) {
    let app = create_router(test_state(pool));
    let (status, json) = send(app, "GET", "/api/v1/applications/app_doesnotexist").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"], "Application not found: app_doesnotexist");
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn test_delete_application_success(pool: PgPool) {
    let app = create_router(test_state(pool));
    let created = create_test_application(app.clone()).await;
    let public_id = created["data"]["public_id"].as_str().unwrap();

    let (status, _) = send(app, "DELETE", &format!("/api/v1/applications/{public_id}")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_delete_marks_state_as_inactive(pool: PgPool) {
    let app = create_router(test_state(pool));
    let created = create_test_application(app.clone()).await;
    let public_id = created["data"]["public_id"].as_str().unwrap();

    send(app.clone(), "DELETE", &format!("/api/v1/applications/{public_id}")).await;

    let (_, json) = send(app, "GET", &format!("/api/v1/applications/{public_id}")).await;
    assert_eq!(json["data"]["state"], "Inactive");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_delete_application_is_noop_when_already_deleted(pool: PgPool) {
    let app = create_router(test_state(pool));
    let created = create_test_application(app.clone()).await;
    let public_id = created["data"]["public_id"].as_str().unwrap();
    let uri = format!("/api/v1/applications/{public_id}");

    let (first, _) = send(app.clone(), "DELETE", &uri).await;
    assert_eq!(first, StatusCode::NO_CONTENT);

    let (second, _) = send(app.clone(), "DELETE", &uri).await;
    assert_eq!(second, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrations = "./migrations")]
async fn test_delete_application_not_found_is_noop(pool: PgPool) {
    let app = create_router(test_state(pool));
    let (status, _) = send(app, "DELETE", "/api/v1/applications/app_doesnotexist").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

// ---------------------------------------------------------------------------
// Restore
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn test_restore_application_success(pool: PgPool) {
    let app = create_router(test_state(pool));
    let created = create_test_application(app.clone()).await;
    let public_id = created["data"]["public_id"].as_str().unwrap();

    send(app.clone(), "DELETE", &format!("/api/v1/applications/{public_id}")).await;

    let (status, json) = send(app, "POST", &format!("/api/v1/applications/{public_id}/restore")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["success"], true);

    let data = &json["data"];
    assert_application_fields(data);
    assert_eq!(data["public_id"], public_id);
    assert_eq!(data["state"], "Active");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_restore_clears_deleted_at(pool: PgPool) {
    let app = create_router(test_state(pool));
    let created = create_test_application(app.clone()).await;
    let public_id = created["data"]["public_id"].as_str().unwrap();

    send(app.clone(), "DELETE", &format!("/api/v1/applications/{public_id}")).await;
    send(app.clone(), "POST", &format!("/api/v1/applications/{public_id}/restore")).await;

    // A second delete should work (proves deleted_at was cleared)
    let (status, _) = send(app.clone(), "DELETE", &format!("/api/v1/applications/{public_id}")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, json) = send(app, "GET", &format!("/api/v1/applications/{public_id}")).await;
    assert_eq!(json["data"]["state"], "Inactive");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_restore_is_noop_when_already_active(pool: PgPool) {
    let app = create_router(test_state(pool));
    let created = create_test_application(app.clone()).await;
    let public_id = created["data"]["public_id"].as_str().unwrap();

    let (status, json) = send(app, "POST", &format!("/api/v1/applications/{public_id}/restore")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(json["data"]["state"], "Active");
}

#[sqlx::test(migrations = "./migrations")]
async fn test_restore_application_not_found(pool: PgPool) {
    let app = create_router(test_state(pool));
    let (status, json) = send(app, "POST", "/api/v1/applications/app_doesnotexist/restore").await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(json["error"], "Application not found: app_doesnotexist");
}
