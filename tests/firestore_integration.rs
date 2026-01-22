//! Firestore integration tests.
//!
//! These tests require the Firestore emulator to be running.
//! Run with: ./scripts/test-with-emulator.sh
//!
//! The emulator provides a clean state for each test run.

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
    FirestoreDb::new("test-project")
        .await
        .expect("Failed to connect to Firestore emulator")
}

/// Generate a unique athlete ID for test isolation.
fn unique_athlete_id() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
        % 1_000_000_000
}

// ═══════════════════════════════════════════════════════════════════════════
// USER TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_new_user_creation() {
    require_emulator!();

    let db = test_db().await;
    let athlete_id = unique_athlete_id();

    // Initially, user should not exist
    let before = db.get_user(athlete_id).await.unwrap();
    assert!(before.is_none(), "User should not exist before creation");

    // Create user
    let user = User {
        strava_athlete_id: athlete_id,
        email: Some("test@example.com".to_string()),
        firstname: "Test".to_string(),
        lastname: "User".to_string(),
        profile_picture: Some("https://example.com/pic.jpg".to_string()),
        created_at: "2024-01-15T10:00:00Z".to_string(),
        last_active: "2024-01-15T10:00:00Z".to_string(),
    };
    db.upsert_user(&user).await.unwrap();

    // Verify user was created with correct data
    let after = db.get_user(athlete_id).await.unwrap();
    assert!(after.is_some(), "User should exist after creation");

    let fetched = after.unwrap();
    assert_eq!(fetched.strava_athlete_id, athlete_id);
    assert_eq!(fetched.firstname, "Test");
    assert_eq!(fetched.lastname, "User");
    assert_eq!(fetched.email, Some("test@example.com".to_string()));
    assert_eq!(
        fetched.profile_picture,
        Some("https://example.com/pic.jpg".to_string())
    );

    println!("✓ New user created and verified: athlete_id={}", athlete_id);
}

#[tokio::test]
async fn test_user_update_preserves_all_fields() {
    require_emulator!();

    let db = test_db().await;
    let athlete_id = unique_athlete_id();

    // Create initial user
    let user_v1 = User {
        strava_athlete_id: athlete_id,
        email: Some("old@example.com".to_string()),
        firstname: "Old".to_string(),
        lastname: "Name".to_string(),
        profile_picture: None,
        created_at: "2024-01-01T00:00:00Z".to_string(),
        last_active: "2024-01-01T00:00:00Z".to_string(),
    };
    db.upsert_user(&user_v1).await.unwrap();

    // Update user with new data
    let user_v2 = User {
        strava_athlete_id: athlete_id,
        email: Some("new@example.com".to_string()),
        firstname: "New".to_string(),
        lastname: "Person".to_string(),
        profile_picture: Some("https://example.com/new.jpg".to_string()),
        created_at: "2024-01-01T00:00:00Z".to_string(), // Should preserve original
        last_active: "2024-01-15T12:00:00Z".to_string(),
    };
    db.upsert_user(&user_v2).await.unwrap();

    // Verify update
    let fetched = db.get_user(athlete_id).await.unwrap().unwrap();
    assert_eq!(fetched.firstname, "New");
    assert_eq!(fetched.lastname, "Person");
    assert_eq!(fetched.email, Some("new@example.com".to_string()));
    assert_eq!(
        fetched.profile_picture,
        Some("https://example.com/new.jpg".to_string())
    );
    // created_at should match original
    assert_eq!(fetched.created_at, "2024-01-01T00:00:00Z");

    println!("✓ User update verified: athlete_id={}", athlete_id);
}

#[tokio::test]
async fn test_tokens_crud() {
    require_emulator!();

    let db = test_db().await;
    let athlete_id = unique_athlete_id();

    // Initially no tokens
    let before = db.get_tokens(athlete_id).await.unwrap();
    assert!(before.is_none(), "Tokens should not exist initially");

    // Store tokens
    let tokens = UserTokens {
        access_token_encrypted: "encrypted_access_123".to_string(),
        refresh_token_encrypted: "encrypted_refresh_456".to_string(),
        expires_at: "2024-01-15T11:00:00Z".to_string(),
        scopes: vec!["read".to_string(), "activity:read".to_string()],
    };
    db.set_tokens(athlete_id, &tokens).await.unwrap();

    // Verify tokens stored correctly
    let fetched = db.get_tokens(athlete_id).await.unwrap().unwrap();
    assert_eq!(fetched.access_token_encrypted, "encrypted_access_123");
    assert_eq!(fetched.refresh_token_encrypted, "encrypted_refresh_456");
    assert_eq!(fetched.scopes.len(), 2);

    // Delete tokens
    db.delete_tokens(athlete_id).await.unwrap();

    // Verify deleted
    let after_delete = db.get_tokens(athlete_id).await.unwrap();
    assert!(after_delete.is_none(), "Tokens should be deleted");

    println!("✓ Token CRUD verified: athlete_id={}", athlete_id);
}

// ═══════════════════════════════════════════════════════════════════════════
// ACTIVITY TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_new_activity_processing() {
    require_emulator!();

    let db = test_db().await;
    let athlete_id = unique_athlete_id();
    let activity_id = athlete_id + 1000;

    // Create the activity
    let activity = Activity {
        strava_activity_id: activity_id,
        athlete_id,
        name: "Morning Ride at Rancho".to_string(),
        sport_type: "Ride".to_string(),
        start_date: "2024-01-15T08:00:00Z".to_string(),
        distance_meters: 15000.0,
        preserves_visited: vec!["Rancho San Antonio".to_string()],
        source: "webhook".to_string(),
        annotation_added: false,
        processed_at: "2024-01-15T08:30:00Z".to_string(),
    };

    // Create preserve records
    let preserve_records = vec![ActivityPreserve {
        athlete_id,
        activity_id,
        preserve_name: "Rancho San Antonio".to_string(),
        start_date: "2024-01-15T08:00:00Z".to_string(),
        activity_name: "Morning Ride at Rancho".to_string(),
        sport_type: "Ride".to_string(),
    }];

    // Process activity atomically
    let processed = db
        .process_activity_atomic(&activity, &preserve_records)
        .await
        .unwrap();
    assert!(processed, "Activity should be processed as new");

    // Verify activity was stored
    let stored_activity = db.get_activity(activity_id).await.unwrap();
    assert!(stored_activity.is_some(), "Activity should exist");
    let stored = stored_activity.unwrap();
    assert_eq!(stored.name, "Morning Ride at Rancho");
    assert_eq!(stored.preserves_visited, vec!["Rancho San Antonio"]);

    // Verify user stats were updated
    let stats = db.get_user_stats(athlete_id).await.unwrap();
    assert!(stats.is_some(), "Stats should exist after processing");
    let stats = stats.unwrap();
    assert_eq!(stats.total_activities, 1);
    assert_eq!(stats.total_distance_meters, 15000.0);
    assert_eq!(stats.preserves.get("Rancho San Antonio"), Some(&1));
    assert!(stats.processed_activity_ids.contains(&activity_id));

    println!(
        "✓ New activity processed: activity_id={}, athlete_id={}",
        activity_id, athlete_id
    );
}

#[tokio::test]
async fn test_activity_idempotency() {
    require_emulator!();

    let db = test_db().await;
    let athlete_id = unique_athlete_id();
    let activity_id = athlete_id + 2000;

    let activity = Activity {
        strava_activity_id: activity_id,
        athlete_id,
        name: "Duplicate Test".to_string(),
        sport_type: "Run".to_string(),
        start_date: "2024-01-15T09:00:00Z".to_string(),
        distance_meters: 5000.0,
        preserves_visited: vec!["Monte Bello".to_string()],
        source: "webhook".to_string(),
        annotation_added: false,
        processed_at: "2024-01-15T09:30:00Z".to_string(),
    };

    // First processing
    let first = db.process_activity_atomic(&activity, &[]).await.unwrap();
    assert!(first, "First processing should succeed");

    // Second processing (duplicate)
    let second = db.process_activity_atomic(&activity, &[]).await.unwrap();
    assert!(!second, "Second processing should be skipped (idempotent)");

    // Verify stats weren't double-counted
    let stats = db.get_user_stats(athlete_id).await.unwrap().unwrap();
    assert_eq!(
        stats.total_activities, 1,
        "Should count activity only once"
    );
    assert_eq!(
        stats.total_distance_meters, 5000.0,
        "Distance should not be doubled"
    );

    println!(
        "✓ Idempotency verified: activity_id={}, athlete_id={}",
        activity_id, athlete_id
    );
}

#[tokio::test]
async fn test_multiple_activities_accumulate_stats() {
    require_emulator!();

    let db = test_db().await;
    let athlete_id = unique_athlete_id();

    // Process first activity
    let activity1 = Activity {
        strava_activity_id: athlete_id + 3001,
        athlete_id,
        name: "Ride 1".to_string(),
        sport_type: "Ride".to_string(),
        start_date: "2024-01-10T08:00:00Z".to_string(),
        distance_meters: 10000.0,
        preserves_visited: vec!["Rancho San Antonio".to_string()],
        source: "backfill".to_string(),
        annotation_added: false,
        processed_at: "2024-01-15T10:00:00Z".to_string(),
    };
    db.process_activity_atomic(&activity1, &[]).await.unwrap();

    // Process second activity (same preserve)
    let activity2 = Activity {
        strava_activity_id: athlete_id + 3002,
        athlete_id,
        name: "Ride 2".to_string(),
        sport_type: "Ride".to_string(),
        start_date: "2024-01-12T08:00:00Z".to_string(),
        distance_meters: 8000.0,
        preserves_visited: vec!["Rancho San Antonio".to_string()],
        source: "backfill".to_string(),
        annotation_added: false,
        processed_at: "2024-01-15T10:00:00Z".to_string(),
    };
    db.process_activity_atomic(&activity2, &[]).await.unwrap();

    // Process third activity (different preserve, different sport)
    let activity3 = Activity {
        strava_activity_id: athlete_id + 3003,
        athlete_id,
        name: "Hike at Monte Bello".to_string(),
        sport_type: "Hike".to_string(),
        start_date: "2024-01-14T10:00:00Z".to_string(),
        distance_meters: 6000.0,
        preserves_visited: vec!["Monte Bello".to_string()],
        source: "webhook".to_string(),
        annotation_added: false,
        processed_at: "2024-01-15T10:00:00Z".to_string(),
    };
    db.process_activity_atomic(&activity3, &[]).await.unwrap();

    // Verify accumulated stats
    let stats = db.get_user_stats(athlete_id).await.unwrap().unwrap();

    // Total counts
    assert_eq!(stats.total_activities, 3);
    assert_eq!(stats.total_distance_meters, 24000.0); // 10k + 8k + 6k

    // Preserve counts
    assert_eq!(stats.preserves.get("Rancho San Antonio"), Some(&2));
    assert_eq!(stats.preserves.get("Monte Bello"), Some(&1));

    // Sport type counts
    assert_eq!(stats.activities_by_sport.get("Ride"), Some(&2));
    assert_eq!(stats.activities_by_sport.get("Hike"), Some(&1));

    // Distance by sport
    assert_eq!(stats.distance_by_sport.get("Ride"), Some(&18000.0));
    assert_eq!(stats.distance_by_sport.get("Hike"), Some(&6000.0));

    // Processed activity IDs
    assert_eq!(stats.processed_activity_ids.len(), 3);

    println!(
        "✓ Multiple activities accumulated correctly: athlete_id={}",
        athlete_id
    );
}

#[tokio::test]
async fn test_activity_with_multiple_preserves() {
    require_emulator!();

    let db = test_db().await;
    let athlete_id = unique_athlete_id();
    let activity_id = athlete_id + 4000;

    // Activity that spans multiple preserves
    let activity = Activity {
        strava_activity_id: activity_id,
        athlete_id,
        name: "Epic Multi-Preserve Ride".to_string(),
        sport_type: "Ride".to_string(),
        start_date: "2024-01-15T07:00:00Z".to_string(),
        distance_meters: 50000.0,
        preserves_visited: vec![
            "Rancho San Antonio".to_string(),
            "Monte Bello".to_string(),
            "Fremont Older".to_string(),
        ],
        source: "webhook".to_string(),
        annotation_added: true,
        processed_at: "2024-01-15T12:00:00Z".to_string(),
    };

    // Create preserve records for each preserve visited
    let preserve_records: Vec<ActivityPreserve> = activity
        .preserves_visited
        .iter()
        .map(|p| ActivityPreserve {
            athlete_id,
            activity_id,
            preserve_name: p.clone(),
            start_date: activity.start_date.clone(),
            activity_name: activity.name.clone(),
            sport_type: activity.sport_type.clone(),
        })
        .collect();

    db.process_activity_atomic(&activity, &preserve_records)
        .await
        .unwrap();

    // Verify stats
    let stats = db.get_user_stats(athlete_id).await.unwrap().unwrap();

    // Still only one activity
    assert_eq!(stats.total_activities, 1);

    // But each preserve should be counted once
    assert_eq!(stats.preserves.get("Rancho San Antonio"), Some(&1));
    assert_eq!(stats.preserves.get("Monte Bello"), Some(&1));
    assert_eq!(stats.preserves.get("Fremont Older"), Some(&1));

    // Query preserve-specific activities
    let rancho_activities = db
        .get_activities_for_preserve(athlete_id, "Rancho San Antonio")
        .await
        .unwrap();
    assert_eq!(rancho_activities.len(), 1);
    assert_eq!(rancho_activities[0].activity_name, "Epic Multi-Preserve Ride");

    let monte_bello_activities = db
        .get_activities_for_preserve(athlete_id, "Monte Bello")
        .await
        .unwrap();
    assert_eq!(monte_bello_activities.len(), 1);

    println!(
        "✓ Multi-preserve activity handled correctly: activity_id={}",
        activity_id
    );
}

#[tokio::test]
async fn test_preserves_by_year_tracking() {
    require_emulator!();

    let db = test_db().await;
    let athlete_id = unique_athlete_id();

    // Activity in 2024
    let activity_2024 = Activity {
        strava_activity_id: athlete_id + 5001,
        athlete_id,
        name: "2024 Ride".to_string(),
        sport_type: "Ride".to_string(),
        start_date: "2024-06-15T08:00:00Z".to_string(),
        distance_meters: 10000.0,
        preserves_visited: vec!["Rancho San Antonio".to_string()],
        source: "backfill".to_string(),
        annotation_added: false,
        processed_at: "2024-06-15T12:00:00Z".to_string(),
    };
    db.process_activity_atomic(&activity_2024, &[]).await.unwrap();

    // Activity in 2025
    let activity_2025 = Activity {
        strava_activity_id: athlete_id + 5002,
        athlete_id,
        name: "2025 Ride".to_string(),
        sport_type: "Ride".to_string(),
        start_date: "2025-01-10T08:00:00Z".to_string(),
        distance_meters: 8000.0,
        preserves_visited: vec!["Rancho San Antonio".to_string(), "Monte Bello".to_string()],
        source: "webhook".to_string(),
        annotation_added: false,
        processed_at: "2025-01-10T12:00:00Z".to_string(),
    };
    db.process_activity_atomic(&activity_2025, &[]).await.unwrap();

    // Verify year-specific stats
    let stats = db.get_user_stats(athlete_id).await.unwrap().unwrap();

    // 2024 preserves
    let year_2024 = stats.preserves_by_year.get("2024").unwrap();
    assert_eq!(year_2024.get("Rancho San Antonio"), Some(&1));
    assert!(year_2024.get("Monte Bello").is_none());

    // 2025 preserves
    let year_2025 = stats.preserves_by_year.get("2025").unwrap();
    assert_eq!(year_2025.get("Rancho San Antonio"), Some(&1));
    assert_eq!(year_2025.get("Monte Bello"), Some(&1));

    // Total preserves (across all years)
    assert_eq!(stats.preserves.get("Rancho San Antonio"), Some(&2));
    assert_eq!(stats.preserves.get("Monte Bello"), Some(&1));

    println!(
        "✓ Preserves by year tracked correctly: athlete_id={}",
        athlete_id
    );
}
