// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! API input validation security tests.
//!
//! These tests verify that:
//! 1. Input parameters like `preserve` length are validated to prevent DoS.
//! 2. Date formats are validated to prevent unexpected behavior.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::ServiceExt;

mod common;

#[tokio::test]
async fn test_activities_validation_preserve_too_long() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    // Create a string slightly longer than 100 chars
    let long_preserve = "a".repeat(101);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/api/activities?preserve={}", long_preserve))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_activities_validation_invalid_date() {
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
async fn test_activities_validation_valid_date() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    // Valid ISO 8601 date
    let valid_date = "2023-01-01T00:00:00Z";

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(&format!("/api/activities?after={}", valid_date))
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should be OK (200) or at least not 400 Bad Request
    // Since we are using a mock DB, it will return 500 because the DB is offline.
    // Ideally we'd mock the DB response to return 200, but for validation testing,
    // proving it passed validation (didn't return 400) is sufficient.
    assert_ne!(response.status(), StatusCode::BAD_REQUEST);
}
