// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Integration tests for preserve intersection detection.
//!
//! Uses real activity fixtures and real preserve boundaries to validate
//! the intersection logic produces correct results.

use midpen_strava::services::PreserveService;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Expected preserves for each fixture activity.
#[derive(Debug, Deserialize)]
struct ExpectedPreserves {
    activities: HashMap<String, ActivityExpected>,
}

#[derive(Debug, Deserialize)]
struct ActivityExpected {
    expected_preserves: Vec<String>,
    // notes field removed
}

/// Simple activity structure for loading fixtures.
#[derive(Debug, Deserialize)]
struct FixtureActivity {
    // id field removed
    map: FixtureMap,
}

#[derive(Debug, Deserialize)]
struct FixtureMap {
    polyline: Option<String>,
    summary_polyline: Option<String>,
}

impl FixtureActivity {
    fn get_polyline(&self) -> Option<&str> {
        self.map
            .polyline
            .as_deref()
            .or(self.map.summary_polyline.as_deref())
    }
}

#[test]
fn test_fixture_preserve_intersections() {
    // Load preserve boundaries
    let geo_path = Path::new("data/midpen_boundaries.geojson");
    if !geo_path.exists() {
        println!(
            "⚠️  Skipping test: Preserve boundaries not found at {:?}",
            geo_path
        );
        return;
    }

    let preserve_service =
        PreserveService::load_from_file(geo_path).expect("Failed to load preserve boundaries");

    println!("Loaded {} preserves", preserve_service.preserves().len());

    // Load expected results
    let expected_path = Path::new("tests/fixtures/expected_preserves.json");
    if !expected_path.exists() {
        println!(
            "⚠️  Skipping test: Expected preserves not found at {:?}",
            expected_path
        );
        return;
    }

    let expected_json =
        fs::read_to_string(expected_path).expect("Failed to read expected preserves");
    let expected: ExpectedPreserves =
        serde_json::from_str(&expected_json).expect("Failed to parse expected preserves");

    // Test each fixture
    let fixtures_dir = Path::new("tests/fixtures");
    let mut tested = 0;
    let mut passed = 0;

    for entry in fs::read_dir(fixtures_dir).expect("Failed to read fixtures dir") {
        let entry = entry.expect("Failed to read dir entry");
        let path = entry.path();

        // Skip non-activity files
        if !path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("activity_"))
            .unwrap_or(false)
        {
            continue;
        }

        // Extract activity ID from filename
        let filename = path.file_stem().unwrap().to_str().unwrap();
        let activity_id = filename.strip_prefix("activity_").unwrap();

        // Load activity fixture
        let activity_json =
            fs::read_to_string(&path).expect(&format!("Failed to read fixture: {:?}", path));
        let activity: FixtureActivity = serde_json::from_str(&activity_json)
            .expect(&format!("Failed to parse fixture: {:?}", path));

        // Get expected preserves for this activity
        let expected_for_activity = expected.activities.get(activity_id);
        if expected_for_activity.is_none() {
            println!(
                "⚠️  No expected preserves defined for activity {}",
                activity_id
            );
            continue;
        }
        let expected_preserves = &expected_for_activity.unwrap().expected_preserves;

        // Run intersection detection
        let polyline = activity.get_polyline();
        if polyline.is_none() {
            println!("⚠️  Activity {} has no polyline, skipping", activity_id);
            continue;
        }

        let detected = preserve_service
            .find_intersections_from_polyline(polyline.unwrap())
            .expect("Failed to detect intersections");

        tested += 1;

        // Compare results
        let mut detected_sorted = detected.clone();
        let mut expected_sorted = expected_preserves.clone();
        detected_sorted.sort();
        expected_sorted.sort();

        if detected_sorted == expected_sorted {
            println!("✅ Activity {}: {:?}", activity_id, detected);
            passed += 1;
        } else {
            println!(
                "❌ Activity {}: expected {:?}, got {:?}",
                activity_id, expected_preserves, detected
            );
        }
    }

    println!("\nResults: {}/{} tests passed", passed, tested);
    assert_eq!(passed, tested, "Some fixture tests failed");
}

#[test]
fn test_preserve_loader() {
    let geo_path = Path::new("data/midpen_boundaries.geojson");
    if !geo_path.exists() {
        println!("⚠️  Skipping test: Preserve boundaries not found");
        return;
    }

    let service = PreserveService::load_from_file(geo_path).expect("Failed to load preserves");

    let preserves = service.preserves();

    // Should have loaded some preserves
    assert!(!preserves.is_empty(), "No preserves loaded");

    // Each preserve should have a name
    for preserve in preserves {
        assert!(!preserve.name.is_empty(), "Preserve has empty name");
    }

    println!("Loaded {} preserves successfully", preserves.len());
}
