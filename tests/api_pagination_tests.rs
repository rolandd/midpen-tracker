// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! API pagination security tests.
//!
//! These tests verify that:
//! 1. Pagination parameters are validated correctly
//! 2. Integer underflows/overflows are prevented

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::ServiceExt;

mod common;

#[tokio::test]
async fn test_pagination_underflow() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    // Request with page=0, which would cause underflow (0-1) in vulnerable code
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/activities?page=0&per_page=10")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();

    // Expect 400 Bad Request
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_pagination_overflow() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    // Request with large page number that causes overflow:
    // page = 50,000,000, per_page = 100
    // (50,000,000 - 1) * 100 = 4,999,999,900 > u32::MAX (4,294,967,295)
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/activities?page=50000000&per_page=100")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();

    // Expect 400 Bad Request
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "Should return 400 Bad Request on pagination overflow"
    );
}
