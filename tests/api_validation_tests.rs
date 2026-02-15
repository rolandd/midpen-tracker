// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! API input validation security tests.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::ServiceExt;

mod common;

#[tokio::test]
async fn test_preserve_name_too_long() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    let long_preserve = "a".repeat(101); // 101 characters

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
async fn test_invalid_date_format() {
    let (app, state) = common::create_test_app();
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/activities?after=invalid-date")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
