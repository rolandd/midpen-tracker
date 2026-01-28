// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

use midpen_tracker::models::Activity;
use midpen_tracker::services::strava::StravaActivity;
use std::fs;
use std::path::Path;

#[test]
fn test_device_name_parsing() {
    let fixture_path = Path::new("tests/fixtures/activity_16804567307.json");
    let json = fs::read_to_string(fixture_path).expect("Failed to read fixture");

    // Test StravaActivity parsing
    let strava_activity: StravaActivity =
        serde_json::from_str(&json).expect("Failed to parse StravaActivity");

    assert_eq!(strava_activity.id, 16804567307);
    assert_eq!(
        strava_activity.device_name.as_deref(),
        Some("Garmin epix Pro (Gen 2) 47mm"),
        "StravaActivity device_name mismatch"
    );

    // Test Activity struct compatibility (manual instantiation)
    let activity = Activity {
        strava_activity_id: strava_activity.id,
        athlete_id: 123,
        name: strava_activity.name.clone(),
        sport_type: strava_activity.sport_type.clone(),
        start_date: strava_activity.start_date.clone(),
        distance_meters: strava_activity.distance,
        preserves_visited: vec![],
        source: "test".to_string(),
        annotation_added: false,
        processed_at: "now".to_string(),
        device_name: strava_activity.device_name.clone(),
    };

    assert_eq!(
        activity.device_name.as_deref(),
        Some("Garmin epix Pro (Gen 2) 47mm"),
        "Activity device_name mismatch"
    );
}
