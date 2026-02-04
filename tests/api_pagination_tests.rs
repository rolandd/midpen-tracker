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
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
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

#[tokio::test]
async fn test_pagination_underflow() {
    let (app, state) = common::create_test_app();
    let token = create_test_jwt(12345, &state.config.jwt_signing_key);

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
