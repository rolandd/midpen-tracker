// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@kernel.org>

//! Integration tests for user deletion.
//!
//! These tests require the Firestore emulator to be running.
//! Run with: ./scripts/test-with-emulator.sh --test user_deletion_tests

use midpen_strava::db::FirestoreDb;
use midpen_strava::models::user::{User, UserTokens};
use midpen_strava::models::{Activity, ActivityPreserve};

/// Check if emulator is available via environment variable.
fn emulator_available() -> bool {
    std::env::var("FIRESTORE_EMULATOR_HOST").is_ok()
}

/// Skip test with message if emulator not available.
macro_rules! require_emulator {
    () => {
        if !emulator_available() {
            eprintln!("⚠️  Skipping: FIRESTORE_EMULATOR_HOST not set");
            eprintln!("   Run with: ./scripts/test-with-emulator.sh");
            return;
        }
    };
}

/// Create a test database connection.
async fn test_db() -> FirestoreDb {
    let project_id = "test-project";
    FirestoreDb::new(project_id).await.unwrap()
}

/// Generate a unique athlete ID for test isolation.
fn unique_athlete_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap();
    since_the_epoch.as_nanos() as u64
}

#[tokio::test]
async fn test_delete_user_data_removes_all_records() {
    require_emulator!();
    let db = test_db().await;
    let athlete_id = unique_athlete_id();
    let now = chrono::Utc::now().to_rfc3339();

    // 1. Create User
    let user = User {
        strava_athlete_id: athlete_id,
        email: None,
        firstname: "Delete".to_string(),
        lastname: "Me".to_string(),
        profile_picture: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        last_active: "2024-01-01T00:00:00Z".to_string(),
        deletion_requested_at: None,
    };
    db.upsert_user(&user).await.unwrap();

    // 2. Create Tokens
    let tokens = UserTokens {
        access_token_encrypted: "encrypted_access".to_string(),
        refresh_token_encrypted: "encrypted_refresh".to_string(),
        expires_at: now.clone(),
        scopes: vec!["read".to_string()],
    };
    db.set_tokens(athlete_id, &tokens).await.unwrap();

    // 3. Create Activity
    let activity = Activity {
        strava_activity_id: 1001,
        athlete_id,
        name: "Run to be deleted".to_string(),
        distance_meters: 5000.0,
        preserves_visited: vec!["Rancho San Antonio".to_string()],
        sport_type: "Run".to_string(),
        start_date: now.clone(),
        source: "test".to_string(),
        annotation_added: false,
        processed_at: now.clone(),
        device_name: None,
    };
    db.set_activity(&activity).await.unwrap();

    // 4. Create Activity Preserve Join
    let preserve = ActivityPreserve {
        activity_id: 1001,
        athlete_id,
        preserve_name: "Rancho San Antonio".to_string(),
        activity_name: "Run".to_string(),
        sport_type: "Run".to_string(),
        start_date: now.clone(),
    };
    db.batch_set_activity_preserves(&[preserve]).await.unwrap();

    // 5. Create User Stats
    let mut stats = midpen_strava::models::UserStats::default();
    stats.processed_activity_ids.insert(1001);
    db.set_user_stats(athlete_id, &stats).await.unwrap();

    // Verify everything exists before deletion
    assert!(db.get_user(athlete_id).await.unwrap().is_some());
    assert!(db.get_tokens(athlete_id).await.unwrap().is_some());
    assert!(db.get_activity(1001).await.unwrap().is_some());
    assert!(!db
        .get_activities_for_preserve(athlete_id, "Rancho San Antonio")
        .await
        .unwrap()
        .is_empty());
    assert!(db.get_user_stats(athlete_id).await.unwrap().is_some());

    // 6. Execute Deletion (GDPR method)
    // Note: This does NOT delete tokens (caller responsibility), only user data
    let count = db.delete_user_data(athlete_id).await.unwrap();
    assert!(count >= 3); // activity + preserve + stats + user

    // Verify Tokens are STILL THERE (caller must delete explicitly)
    assert!(db.get_tokens(athlete_id).await.unwrap().is_some());

    // Verify Everything Else is GONE
    assert!(db.get_user(athlete_id).await.unwrap().is_none());
    assert!(db.get_activity(1001).await.unwrap().is_none());
    assert!(db
        .get_activities_for_preserve(athlete_id, "Rancho San Antonio")
        .await
        .unwrap()
        .is_empty());
    assert!(db.get_user_stats(athlete_id).await.unwrap().is_none());

    // 7. Explicitly delete tokens (simulating task handler)
    db.delete_tokens(athlete_id).await.unwrap();
    assert!(db.get_tokens(athlete_id).await.unwrap().is_none());
}
