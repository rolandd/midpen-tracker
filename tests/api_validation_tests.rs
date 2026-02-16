// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! API input validation tests.
//!
//! These tests verify that:
//! 1. Input parameters like 'preserve' are length-limited
//! 2. Date strings like 'after' are format-validated

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::ServiceExt;

mod common;

#[tokio::test]
async fn test_activities_preserve_too_long() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    // Create a 101 character string
    let long_preserve = "a".repeat(101);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/activities?preserve={}", long_preserve))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_activities_after_invalid_date() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/activities?after=not-a-date")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_activities_valid_input() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/activities?after=2025-01-01T00:00:00Z&preserve=Rancho")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should pass validation (but fail at DB layer with 500 due to offline mock)
    // The key is that it's NOT Bad Request (400)
    assert_ne!(response.status(), StatusCode::BAD_REQUEST);
}
