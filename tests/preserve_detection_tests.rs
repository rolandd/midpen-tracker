// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Preserve detection smoke tests.
//!
//! These tests verify that the polyline → preserve detection pipeline works.
//! For detailed regression testing with known polylines, see preserve_integration.rs.
//!
//! IMPORTANT: If these tests fail, it indicates breakage in the core preserve
//! matching logic that users won't notice until they check their stats.

use midpen_tracker::services::PreserveService;

/// Load the real preserve boundaries for testing.
fn load_test_preserves() -> PreserveService {
    PreserveService::load_from_file("data/midpen_boundaries.geojson")
        .expect("Failed to load preserve boundaries - is data/ committed?")
}

#[test]
fn test_preserve_service_loads() {
    let service = load_test_preserves();
    let count = service.preserves().len();

    // Should have loaded multiple preserves
    assert!(count > 0, "Should load at least one preserve");
    // We expect exactly 25 preserves (28 total - 3 closed/null-url)
    assert_eq!(count, 25, "Expected exactly 25 preserves, got {}", count);

    // Spot check some expected preserve names
    let names: Vec<&str> = service
        .preserves()
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert!(
        names.iter().any(|n| n.contains("Rancho")),
        "Should have Rancho preserve"
    );
    assert!(
        names.iter().any(|n| n.contains("Monte Bello")),
        "Should have Monte Bello preserve"
    );
    assert!(
        names.iter().any(|n| n.contains("Windy")),
        "Should have Windy Hill preserve"
    );

    // Verify closed preserves are NOT present
    let closed_preserves = ["Felton Station", "Miramontes Ridge", "Tunitas Creek"];
    for closed in closed_preserves {
        assert!(
            !names.iter().any(|n| *n == closed),
            "Should NOT have closed preserve: {}",
            closed
        );
    }
}

#[test]
fn test_sf_downtown_no_match() {
    let service = load_test_preserves();

    // A polyline in downtown San Francisco (definitely not in any preserve)
    // This is a simple line from near Market Street
    let sf_downtown_polyline = "gn~eFhmdjVs@gAqAeBuAgB";

    let matches = service
        .find_intersections_from_polyline(sf_downtown_polyline)
        .expect("Failed to decode polyline");

    // SF downtown should NOT match any preserve
    assert!(
        matches.is_empty(),
        "SF downtown polyline should not match any preserves, found: {:?}",
        matches
    );
}

#[test]
fn test_invalid_polyline_error() {
    let service = load_test_preserves();

    // Invalid polylines should return an error, not panic
    let result = service.find_intersections_from_polyline("invalid!!!");
    assert!(result.is_err(), "Invalid polyline should return error");
}

#[test]
fn test_empty_polyline_handling() {
    let service = load_test_preserves();

    // Empty polyline - should not panic
    let result = service.find_intersections_from_polyline("");
    // Either error or empty matches is acceptable
    if let Ok(matches) = result {
        assert!(matches.is_empty(), "Empty polyline should match nothing");
    }
}

#[test]
fn test_line_intersection_basic() {
    // Test the underlying find_intersections method directly with known coordinates
    let service = load_test_preserves();

    // Create a line string that's definitely outside California
    // (should not match any preserve)
    let line_outside = geo::LineString::from(vec![
        (-73.985, 40.748), // New York City
        (-73.975, 40.758),
    ]);

    let matches = service.find_intersections(&line_outside);
    assert!(
        matches.is_empty(),
        "NYC line should not match any CA preserve"
    );
}

#[test]
fn test_all_preserves_have_valid_geometry() {
    let service = load_test_preserves();

    // Every loaded preserve should have a name and valid geometry
    for preserve in service.preserves() {
        assert!(
            !preserve.name.is_empty(),
            "Preserve name should not be empty"
        );
        // The geometry is validated during loading, so if we got here it's valid
    }
}

#[test]
fn test_preserve_names_unique() {
    let service = load_test_preserves();
    let names: Vec<&str> = service
        .preserves()
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    // Check for duplicate names (which would indicate a data problem)
    let mut seen = std::collections::HashSet::new();
    for name in &names {
        assert!(seen.insert(*name), "Duplicate preserve name: {}", name);
    }
}
