// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! API authentication and CORS tests.
//!
//! These tests verify that:
//! 1. Protected routes reject requests without valid tokens
//! 2. Protected routes accept requests with valid tokens  
//! 3. CORS preflight requests return correct headers

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use tower::ServiceExt;

mod common;

#[tokio::test]
async fn test_protected_route_without_token() {
    let (app, _) = common::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/stats/preserves")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 401 Unauthorized without token
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_route_with_invalid_token() {
    let (app, _) = common::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/stats/preserves")
                .header(header::AUTHORIZATION, "Bearer invalid.token.here")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 401 Unauthorized with invalid token
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_protected_route_with_valid_token() {
    let (app, state) = common::create_test_app().await;
    let token = common::create_test_jwt(12345, &state.config.jwt_signing_key);

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/stats/preserves")
                .header(header::AUTHORIZATION, format!("Bearer {}", token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // With valid token: 200 if Firestore available, 500 if Firestore unavailable (GCP feature without emulator)
    // The key check is that we DON'T get 401 (authentication succeeded)
    let status = response.status();
    assert!(
        status == StatusCode::OK || status == StatusCode::INTERNAL_SERVER_ERROR,
        "Expected 200 or 500, got {}. Auth should pass, Firestore may fail without emulator.",
        status
    );
}

#[tokio::test]
async fn test_cors_preflight() {
    let (app, _) = common::create_test_app().await;

    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/stats/preserves")
                .header(header::ORIGIN, "http://localhost:5173")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // OPTIONS should return 200 (CORS preflight success)
    assert_eq!(response.status(), StatusCode::OK);

    // Should have CORS headers
    assert!(response
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    assert!(response
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_METHODS));
}

#[tokio::test]
async fn test_public_route_no_auth_required() {
    let (app, _) = common::create_test_app().await;

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

    // Health should be accessible without auth
    assert_eq!(response.status(), StatusCode::OK);
}
