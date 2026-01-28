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
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;

mod common;

/// Create a test JWT token.
fn create_test_jwt(athlete_id: u64, signing_key: &[u8]) -> String {
    #[derive(Serialize)]
    struct Claims {
        sub: String,
        exp: usize,
        iat: usize,
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = Claims {
        sub: athlete_id.to_string(),
        exp: now + 86400,
        iat: now,
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(signing_key),
    )
    .unwrap()
}

/// Create a test app with known signing key.
async fn create_test_app() -> (axum::Router, Vec<u8>) {
    use midpen_tracker::config::Config;
    use midpen_tracker::routes::create_router;
    use midpen_tracker::services::{PreserveService, TasksService};
    use midpen_tracker::AppState;

    let config = Config::test_default();
    let signing_key = config.jwt_signing_key.clone();

    let db = common::test_db_offline();
    let preserve_service = PreserveService::default();
    let tasks_service = TasksService::new(&config.gcp_project_id, &config.gcp_region);

    let state = Arc::new(AppState {
        config,
        db,
        preserve_service,
        tasks_service,
    });

    (create_router(state), signing_key)
}

#[tokio::test]
async fn test_protected_route_without_token() {
    let (app, _) = create_test_app().await;

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
    let (app, _) = create_test_app().await;

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
    let (app, signing_key) = create_test_app().await;
    let token = create_test_jwt(12345, &signing_key);

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
    let (app, _) = create_test_app().await;

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
    let (app, _) = create_test_app().await;

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
