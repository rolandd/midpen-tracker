// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Tests for token cache invalidation.

use chrono::Utc;
use dashmap::DashMap;
use midpen_tracker::models::UserTokens;
use midpen_tracker::services::{KmsService, StravaService};
use std::sync::Arc;

mod common;
use common::test_db;

#[tokio::test]
async fn test_revoke_local_tokens_invalidates_cache() {
    require_emulator!();
    let db = test_db().await;

    // Setup Service
    let kms = KmsService::new_mock();
    let token_cache = Arc::new(DashMap::new());

    // We can pass dummy client keys since we won't hit real Strava API
    let strava_service = StravaService::new(
        "client_id".to_string(),
        "client_secret".to_string(),
        db.clone(),
        kms.clone(),
        token_cache.clone(),
    );

    let athlete_id = 99999;

    // 1. Seed DB with Tokens (using mock encryption)
    let access_token_plain = "valid_access_token";
    let refresh_token_plain = "valid_refresh_token";

    // Manual mock encryption (base64) matching our KmsService mock
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    let enc_access = BASE64.encode(access_token_plain);
    let enc_refresh = BASE64.encode(refresh_token_plain);

    let tokens = UserTokens {
        access_token_encrypted: enc_access,
        refresh_token_encrypted: enc_refresh,
        expires_at: (Utc::now() + chrono::Duration::hours(1)).to_rfc3339(),
        scopes: vec!["read".to_string()],
    };
    db.set_tokens(athlete_id, &tokens).await.unwrap();

    // 2. Warm the Cache
    // This calls get_valid_access_token, which fetches from DB, decrypts (mock), and caches
    let result = strava_service.get_valid_access_token(athlete_id).await;
    assert_eq!(
        result.unwrap(),
        access_token_plain,
        "Should return token from DB"
    );

    // Verify it is in cache
    assert!(
        token_cache.contains_key(&athlete_id),
        "Token should be in cache"
    );

    // 3. Revoke Tokens
    // This should delete from DB AND remove from cache
    strava_service
        .revoke_local_tokens(athlete_id)
        .await
        .unwrap();

    // 4. Verify Cache is Empty
    assert!(
        !token_cache.contains_key(&athlete_id),
        "Cache should be invalidated immediately"
    );

    // 5. Verify get_valid_access_token fails
    let result_after = strava_service.get_valid_access_token(athlete_id).await;
    assert!(
        result_after.is_err(),
        "Should fail as tokens are gone from DB and Cache"
    );
}
