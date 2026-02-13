// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Security tests for Cloud Task handlers.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

mod common;

#[tokio::test]
async fn test_process_activity_no_header_forbidden() {
    let (app, _) = common::create_test_app().await;

    let payload = json!({
        "activity_id": 12345,
        "athlete_id": 67890,
        "source": "test"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/process-activity")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_process_activity_missing_auth_forbidden() {
    let (app, _) = common::create_test_app().await;

    let payload = json!({
        "activity_id": 12345,
        "athlete_id": 67890,
        "source": "test"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/process-activity")
                .header("content-type", "application/json")
                .header("x-cloudtasks-queuename", "activity-processing")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_process_activity_with_header_allowed() {
    let (app, state) = common::create_test_app().await;
    let token = common::create_test_tasks_oidc_jwt(&state.config);

    let payload = json!({
        "activity_id": 12345,
        "athlete_id": 67890,
        "source": "test"
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

    // Should NOT be Forbidden.
    // It will likely be INTERNAL_SERVER_ERROR because Strava service fails in test env
    // or OK if it mocks out early.
    // The key is that it passed the security check.
    assert_ne!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_process_activity_wrong_queue_name_forbidden() {
    let (app, state) = common::create_test_app().await;
    let token = common::create_test_tasks_oidc_jwt(&state.config);

    let payload = json!({
        "activity_id": 12345,
        "athlete_id": 67890,
        "source": "test"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/process-activity")
                .header("content-type", "application/json")
                .header("x-cloudtasks-queuename", "wrong-queue")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_continue_backfill_no_header_forbidden() {
    let (app, _) = common::create_test_app().await;

    let payload = json!({
        "athlete_id": 67890,
        "next_page": 2,
        "after_timestamp": 1234567890
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/continue-backfill")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
