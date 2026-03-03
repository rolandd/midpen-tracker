// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Integration tests for parameter validation.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

mod common;

#[tokio::test]
async fn test_activities_validation_invalid_page() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/activities?page=0")
                .header("cookie", format!("midpen_token={}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_activities_validation_per_page_too_large() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/activities?per_page=101")
                .header("cookie", format!("midpen_token={}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_activities_validation_invalid_date_format() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/activities?after=not-a-date")
                .header("cookie", format!("midpen_token={}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_activities_validation_preserve_name_too_long() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    let long_name = "a".repeat(101);
    let uri = format!("/api/activities?preserve={}", long_name);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header("cookie", format!("midpen_token={}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_webhook_verify_validation_invalid_mode() {
    let (app, state) = common::create_test_app();
    let uuid = &state.config.webhook_path_uuid;

    let long_mode = "a".repeat(21);
    let uri = format!(
        "/webhook/{}?hub.mode={}&hub.challenge=test&hub.verify_token=test",
        uuid, long_mode
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_webhook_event_validation_invalid_type() {
    let (app, state) = common::create_test_app();
    let uuid = &state.config.webhook_path_uuid;

    let payload = json!({
        "object_type": "this_type_is_definitely_too_long_to_be_valid_according_to_our_rules",
        "object_id": 123,
        "aspect_type": "create",
        "owner_id": 456,
        "subscription_id": state.config.strava_subscription_id
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/webhook/{}", uuid))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // handle_event returns 200 even on validation error to satisfy Strava,
    // but it should log an error and not process it.
    // However, our current code returns StatusCode::OK for validation errors in handle_event.
    // Let's verify it doesn't crash.
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_global_body_limit() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_tasks_oidc_jwt(&state.config);

    // 64KB + 1 byte
    let large_body = vec![0u8; 64 * 1024 + 1];

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/process-activity")
                .header("authorization", format!("Bearer {token}"))
                .header("x-cloudtasks-queuename", "activity-processing")
                .header("content-type", "application/json")
                .body(Body::from(large_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn test_tasks_validation_invalid_source() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_tasks_oidc_jwt(&state.config);

    let payload = json!({
        "activity_id": 12345,
        "athlete_id": 67890,
        "source": "a".repeat(21)
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/process-activity")
                .header("content-type", "application/json")
                .header("x-cloudtasks-queuename", "activity-processing")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Task handlers return OK even on validation error to stop Cloud Tasks from retrying
    // an invalid payload.
    assert_eq!(response.status(), StatusCode::OK);
}
