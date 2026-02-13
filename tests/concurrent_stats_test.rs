use midpen_tracker::models::Activity;

mod common;
use common::test_db;

const NUM_CONCURRENT_ACTIVITIES: u64 = 10;
const ACTIVITY_DISTANCE: f64 = 100.0;

#[tokio::test]
async fn test_concurrent_activity_processing_race_condition() {
    // This test attempts to reproduce the race condition where stats are read outside the transaction.
    // If stats are read outside, two concurrent processes might read the same initial stats,
    // both increment it, and then write back. One increment would be lost.

    if std::env::var("FIRESTORE_EMULATOR_HOST").is_err() {
        println!("Skipping test because FIRESTORE_EMULATOR_HOST is not set");
        return;
    }

    let db = test_db().await;
    let athlete_id = 123456789;

    // Create user
    let user = midpen_tracker::models::User {
        strava_athlete_id: athlete_id,
        email: Some("race@example.com".to_string()),
        firstname: "Race".to_string(),
        lastname: "Condition".to_string(),
        profile_picture: None,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_active: chrono::Utc::now().to_rfc3339(),
        deletion_requested_at: None,
    };
    db.upsert_user(&user)
        .await
        .expect("Failed to create test user");

    let mut handles = vec![];

    for i in 0..NUM_CONCURRENT_ACTIVITIES {
        let db_clone = db.clone();
        handles.push(tokio::spawn(async move {
            let activity_id = 1000 + i;
            let activity = Activity {
                strava_activity_id: activity_id,
                athlete_id,
                name: format!("Race Activity {}", i),
                sport_type: "Run".to_string(),
                start_date: "2024-01-01T10:00:00Z".to_string(),
                distance_meters: ACTIVITY_DISTANCE,
                preserves_visited: vec![],
                source: "test".to_string(),
                annotation_added: false,
                processed_at: chrono::Utc::now().to_rfc3339(),
                device_name: None,
            };

            db_clone.process_activity_atomic(&activity, &[]).await
        }));
    }

    // Wait for all
    for handle in handles {
        handle
            .await
            .expect("Task join failed")
            .expect("Activity processing failed");
    }

    // Check stats
    let stats = db
        .get_user_stats(athlete_id)
        .await
        .expect("Failed to fetch user stats")
        .expect("User stats document not found");

    assert_eq!(
        stats.total_activities, NUM_CONCURRENT_ACTIVITIES as u32,
        "Total activities count mismatch due to race condition"
    );
    assert_eq!(
        stats.total_distance_meters,
        (NUM_CONCURRENT_ACTIVITIES as f64) * ACTIVITY_DISTANCE,
        "Total distance mismatch due to race condition"
    );
}
