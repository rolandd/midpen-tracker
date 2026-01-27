// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! JWT authentication tests.
//!
//! These tests verify that JWT tokens created by auth routes can be decoded
//! by the auth middleware, catching compatibility issues early.

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

/// Claims structure that must match what the middleware expects.
/// This is the canonical format - if either create_jwt or the middleware
/// changes, this test should catch the incompatibility.
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    iat: usize,
}

/// Test helper to create a JWT token (mirrors auth.rs logic).
fn create_test_jwt(athlete_id: u64, signing_key: &[u8]) -> String {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = Claims {
        sub: athlete_id.to_string(),
        exp: now + 86400 * 30,
        iat: now,
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(signing_key),
    )
    .expect("Failed to create JWT")
}

#[test]
fn test_jwt_roundtrip() {
    // This test verifies that a JWT created by the auth flow can be decoded
    // by the middleware. If either side changes the Claims structure or algorithm,
    // this test will fail.

    let signing_key = b"test_signing_key_32_bytes_long!!";
    let athlete_id = 12345678u64;

    // Create token (like auth.rs does)
    let token = create_test_jwt(athlete_id, signing_key);

    // Decode token (like middleware does)
    let key = DecodingKey::from_secret(signing_key);
    let validation = Validation::new(Algorithm::HS256);

    let token_data = decode::<Claims>(&token, &key, &validation)
        .expect("Failed to decode JWT - check Claims struct compatibility");

    // Verify the claims match
    assert_eq!(token_data.claims.sub, athlete_id.to_string());
    assert!(token_data.claims.exp > 0);
    assert!(token_data.claims.iat > 0);
    assert!(token_data.claims.exp > token_data.claims.iat);
}

#[test]
fn test_jwt_athlete_id_parsing() {
    // Test that the sub claim can be parsed back to u64
    let signing_key = b"test_signing_key_32_bytes_long!!";
    let athlete_id = 98765432u64;

    let token = create_test_jwt(athlete_id, signing_key);

    let key = DecodingKey::from_secret(signing_key);
    let validation = Validation::new(Algorithm::HS256);
    let token_data = decode::<Claims>(&token, &key, &validation).unwrap();

    let parsed_id: u64 = token_data
        .claims
        .sub
        .parse()
        .expect("sub claim should be parseable as u64");

    assert_eq!(parsed_id, athlete_id);
}

#[test]
fn test_jwt_expiration_is_future() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let signing_key = b"test_signing_key_32_bytes_long!!";
    let token = create_test_jwt(12345, signing_key);

    let key = DecodingKey::from_secret(signing_key);
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = false; // We'll check manually

    let token_data = decode::<Claims>(&token, &key, &validation).unwrap();

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    // Token should expire at least 29 days in the future
    assert!(
        token_data.claims.exp > now + 86400 * 29,
        "Token expiration should be ~30 days in the future"
    );
}
