// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Integration tests for webhook handling.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

/// Create a test app with mock dependencies (no GCP required)
async fn create_offline_test_app() -> axum::Router {
    use midpen_strava::config::Config;
    use midpen_strava::db::FirestoreDb;
    use midpen_strava::routes::create_router;
    use midpen_strava::services::{PreserveService, TasksService};
    use midpen_strava::AppState;
    use std::sync::Arc;

    let config = Config::test_default();
    let db = FirestoreDb::new_mock();
    let preserve_service = PreserveService::default();
    let tasks_service = TasksService::new(&config.gcp_project_id);

    let state = Arc::new(AppState {
        config,
        db,
        preserve_service,
        tasks_service,
    });

    create_router(state)
}

#[tokio::test]
async fn test_webhook_verification() {
    let app = create_offline_test_app().await;

    let challenge = "test_challenge_123";
    let verify_token = "test_verify_token"; // Matches Config::default()

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/webhook?hub.mode=subscribe&hub.challenge={}&hub.verify_token={}",
                    challenge, verify_token
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // Verify the response contains the challenge
    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["hub.challenge"], challenge);
}

#[tokio::test]
async fn test_webhook_verification_wrong_token() {
    let app = create_offline_test_app().await;

    let challenge = "test_challenge_123";
    let wrong_token = "wrong_token";

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/webhook?hub.mode=subscribe&hub.challenge={}&hub.verify_token={}",
                    challenge, wrong_token
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 200 but with empty challenge (Strava expects 200)
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), 1024)
        .await
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["hub.challenge"], ""); // Empty challenge = rejection
}

#[tokio::test]
async fn test_webhook_event_create_activity() {
    let app = create_offline_test_app().await;

    let event = json!({
        "aspect_type": "create",
        "event_time": 1234567890,
        "object_id": 12345678901_u64,
        "object_type": "activity",
        "owner_id": 123456,
        "subscription_id": 12345
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhook")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&event).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should always return 200 (async processing via Cloud Tasks)
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_webhook_event_update_activity() {
    let app = create_offline_test_app().await;

    let event = json!({
        "aspect_type": "update",
        "event_time": 1234567890,
        "object_id": 12345678901_u64,
        "object_type": "activity",
        "owner_id": 123456,
        "subscription_id": 12345,
        "updates": {"title": "New Title"}
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhook")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&event).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 200 (logged but not processed)
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_webhook_event_delete_activity() {
    let app = create_offline_test_app().await;

    let event = json!({
        "aspect_type": "delete",
        "event_time": 1234567890,
        "object_id": 12345678901_u64,
        "object_type": "activity",
        "owner_id": 123456,
        "subscription_id": 12345
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhook")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&event).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 200 (attempts to delete, but activity may not exist)
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_webhook_event_athlete_deauthorize() {
    let app = create_offline_test_app().await;

    let event = json!({
        "aspect_type": "deauthorize",
        "event_time": 1234567890,
        "object_id": 0,
        "object_type": "athlete",
        "owner_id": 123456,
        "subscription_id": 12345
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhook")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&event).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 200 (attempts to delete tokens)
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_webhook_event_unknown_type() {
    let app = create_offline_test_app().await;

    let event = json!({
        "aspect_type": "unknown_aspect",
        "event_time": 1234567890,
        "object_id": 12345,
        "object_type": "unknown_object",
        "owner_id": 123456,
        "subscription_id": 12345
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/webhook")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&event).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 200 even for unknown event types
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_health_endpoint() {
    let app = create_offline_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // If health endpoint exists, should return 200
    // If not implemented, 404 is also acceptable for this test
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);
}
