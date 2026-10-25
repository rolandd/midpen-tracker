// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Property-based tests for preserve geometry handling.
//!
//! The example-based tests next door (`preserve_detection_tests.rs`,
//! `preserve_integration.rs`) pin down specific polylines and the real
//! boundary file. These tests instead assert invariants over generated
//! geometry:
//!
//! - the bounding-box prefilter in `find_intersections` never changes the
//!   result compared with the brute-force "always run the expensive check"
//!   definition;
//! - results are invariant under translation and under inserting redundant
//!   vertices into the activity line;
//! - the Strava encoded-polyline path agrees with the decoded coordinates;
//! - polygons that contain the line are always detected (so a regression to
//!   "always return nothing" cannot pass the equality properties above);
//! - loading GeoJSON preserves its geometry and bounding rectangle exactly.
//!
//! Coordinates are integers, which keeps exact-tie behaviour deterministic and
//! makes the polyline round-trip below lossless.

use geo::{Coord, LineString};
use midpen_tracker::services::preserve::PreserveError;
use midpen_tracker::services::PreserveService;
use proptest::prelude::*;
use serde_json::{json, Value};

/// Axis-aligned rectangle in integer degree space; always non-degenerate.
#[derive(Debug, Clone)]
struct Rect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl Rect {
    fn ring(&self) -> Vec<Vec<f64>> {
        let (x0, y0, x1, y1) = (
            f64::from(self.x0),
            f64::from(self.y0),
            f64::from(self.x1),
            f64::from(self.y1),
        );
        vec![
            vec![x0, y0],
            vec![x1, y0],
            vec![x1, y1],
            vec![x0, y1],
            vec![x0, y0],
        ]
    }

    fn shifted(&self, dx: i32, dy: i32) -> Rect {
        Rect {
            x0: self.x0 + dx,
            y0: self.y0 + dy,
            x1: self.x1 + dx,
            y1: self.y1 + dy,
        }
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        x > f64::from(self.x0)
            && x < f64::from(self.x1)
            && y > f64::from(self.y0)
            && y < f64::from(self.y1)
    }
}

fn rect_strategy() -> impl Strategy<Value = Rect> {
    (-10i32..10, -10i32..10, 1i32..6, 1i32..6).prop_map(|(x, y, w, h)| Rect {
        x0: x,
        y0: y,
        x1: x + w,
        y1: y + h,
    })
}

fn point_strategy() -> impl Strategy<Value = (i32, i32)> {
    (-12i32..12, -12i32..12)
}

fn points_strategy() -> impl Strategy<Value = Vec<(i32, i32)>> {
    prop::collection::vec(point_strategy(), 1..6)
}

fn feature(name: &str, geometry: Value) -> Value {
    json!({
        "type": "Feature",
        "properties": { "name": name, "url": "https://example.com/preserve" },
        "geometry": geometry
    })
}

fn polygon_geometry(rect: &Rect) -> Value {
    json!({ "type": "Polygon", "coordinates": [rect.ring()] })
}

/// Build a service from rectangles, named `P0..Pn` so names are unique.
fn service_from_rects(rects: &[Rect]) -> PreserveService {
    let features: Vec<Value> = rects
        .iter()
        .enumerate()
        .map(|(i, rect)| feature(&format!("P{i}"), polygon_geometry(rect)))
        .collect();

    PreserveService::load_from_json(
        &json!({ "type": "FeatureCollection", "features": features }).to_string(),
    )
    .expect("generated GeoJSON must load")
}

fn line_from(points: &[(i32, i32)]) -> LineString<f64> {
    LineString::from(
        points
            .iter()
            .map(|(x, y)| (f64::from(*x), f64::from(*y)))
            .collect::<Vec<_>>(),
    )
}

/// The definition `find_intersections` optimises: no bounding-box shortcut.
fn brute_force(service: &PreserveService, line: &LineString<f64>) -> Vec<String> {
    service
        .preserves()
        .iter()
        .filter(|preserve| preserve.geometry.intersects(line))
        .map(|preserve| preserve.name.clone())
        .collect()
}

fn names(result: &[String]) -> Vec<&str> {
    result.iter().map(String::as_str).collect()
}

proptest! {
    // Geometry work is fast, but the loader parses JSON per case; 128 cases
    // still runs well under a second and keeps CI predictable.
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// The bounding-box prefilter is an optimisation, not part of the
    /// semantics: it must never change which preserves are reported.
    #[test]
    fn bbox_prefilter_agrees_with_brute_force(
        rects in prop::collection::vec(rect_strategy(), 0..4),
        points in points_strategy(),
    ) {
        let service = service_from_rects(&rects);
        let line = line_from(&points);

        prop_assert_eq!(
            service.find_intersections(&line),
            brute_force(&service, &line),
            "prefilter disagreed for rects {:?} and line {:?}",
            rects,
            points
        );
    }

    /// Detection is coordinate-relative: moving preserve and activity together
    /// must not change the outcome (catches swapped x/y and offset bugs).
    #[test]
    fn detection_is_translation_invariant(
        rects in prop::collection::vec(rect_strategy(), 0..4),
        points in points_strategy(),
        dx in -5i32..5,
        dy in -5i32..5,
    ) {
        let original = service_from_rects(&rects);
        let shifted_rects: Vec<Rect> = rects.iter().map(|r| r.shifted(dx, dy)).collect();
        let shifted = service_from_rects(&shifted_rects);

        let shifted_points: Vec<(i32, i32)> = points.iter().map(|(x, y)| (x + dx, y + dy)).collect();

        prop_assert_eq!(
            original.find_intersections(&line_from(&points)),
            shifted.find_intersections(&line_from(&shifted_points))
        );
    }

    /// Strava polylines arrive densely sampled; adding redundant vertices
    /// along existing segments must not add or remove preserves.
    #[test]
    fn detection_is_subdivision_invariant(
        rects in prop::collection::vec(rect_strategy(), 0..4),
        points in points_strategy(),
    ) {
        prop_assume!(points.len() >= 2);

        let service = service_from_rects(&rects);

        let mut subdivided: Vec<Coord<f64>> = Vec::new();
        for pair in points.windows(2) {
            let (ax, ay) = (f64::from(pair[0].0), f64::from(pair[0].1));
            let (bx, by) = (f64::from(pair[1].0), f64::from(pair[1].1));
            subdivided.push(Coord { x: ax, y: ay });
            subdivided.push(Coord { x: (ax + bx) / 2.0, y: (ay + by) / 2.0 });
        }
        let last = points[points.len() - 1];
        subdivided.push(Coord { x: f64::from(last.0), y: f64::from(last.1) });

        prop_assert_eq!(
            service.find_intersections(&line_from(&points)),
            service.find_intersections(&LineString::from(subdivided))
        );
    }

    /// A line whose every vertex is strictly inside a preserve is always
    /// detected — the sanity check that the equality properties cannot satisfy
    /// by returning nothing.
    #[test]
    fn line_inside_preserve_is_detected(
        rect in rect_strategy(),
        fractions in prop::collection::vec((0.05f64..0.95, 0.05f64..0.95), 2..4),
    ) {
        let service = service_from_rects(std::slice::from_ref(&rect));

        let points: Vec<(f64, f64)> = fractions
            .iter()
            .map(|(fx, fy)| {
                (
                    f64::from(rect.x0) + f64::from(rect.x1 - rect.x0) * fx,
                    f64::from(rect.y0) + f64::from(rect.y1 - rect.y0) * fy,
                )
            })
            .collect();
        prop_assert!(points.iter().all(|(x, y)| rect.contains(*x, *y)));

        let line = LineString::from(points);
        let result = service.find_intersections(&line);
        prop_assert_eq!(names(&result), vec!["P0"]);
    }

    /// A line that spans a preserve (a run through it) must always be
    /// reported — the shape real activities have. Pins the prefilter to the
    /// conservative side.
    #[test]
    fn lines_crossing_a_preserve_are_reported(
        rect in rect_strategy(),
        fraction in 0.05f64..0.95,
        margin in 1i32..4,
        horizontal in any::<bool>(),
    ) {
        let service = service_from_rects(std::slice::from_ref(&rect));

        let line = if horizontal {
            let y = f64::from(rect.y0) + f64::from(rect.y1 - rect.y0) * fraction;
            LineString::from(vec![
                (f64::from(rect.x0 - margin), y),
                (f64::from(rect.x1 + margin), y),
            ])
        } else {
            let x = f64::from(rect.x0) + f64::from(rect.x1 - rect.x0) * fraction;
            LineString::from(vec![
                (x, f64::from(rect.y0 - margin)),
                (x, f64::from(rect.y1 + margin)),
            ])
        };

        let result = service.find_intersections(&line);
        prop_assert_eq!(names(&result), vec!["P0"]);
        prop_assert_eq!(result, brute_force(&service, &line));
    }

    /// The encoded-polyline path (what Strava actually sends) agrees with the
    /// decoded coordinates, and the codec itself round-trips integer degrees.
    #[test]
    fn encoded_polyline_matches_decoded_line(
        rects in prop::collection::vec(rect_strategy(), 0..4),
        points in prop::collection::vec((-89i32..89, -80i32..80), 1..8),
    ) {
        let service = service_from_rects(&rects);
        let line = line_from(&points);

        let encoded = polyline::encode_coordinates(line.0.clone(), 5)
            .expect("integer degrees are always encodable");
        let decoded = polyline::decode_polyline(&encoded, 5).expect("encoded polyline must decode");

        let decoded_coords: Vec<(f64, f64)> = decoded.0.iter().map(|c| (c.x, c.y)).collect();
        let original_coords: Vec<(f64, f64)> = line.0.iter().map(|c| (c.x, c.y)).collect();
        prop_assert_eq!(decoded_coords, original_coords);

        prop_assert_eq!(
            service.find_intersections_from_polyline(&encoded).expect("decodes"),
            service.find_intersections(&line)
        );
    }

    /// MultiPolygons are a separate code path in `PreserveGeometry`; a line
    /// inside any part must be reported exactly once.
    #[test]
    fn multipolygon_parts_are_all_checked(
        left in rect_strategy(),
        offset in 1i32..10,
        in_left in any::<bool>(),
        fractions in prop::collection::vec((0.05f64..0.95, 0.05f64..0.95), 2..3),
    ) {
        let right = left.shifted(offset, 0);
        let geometry = json!({
            "type": "MultiPolygon",
            "coordinates": [[left.ring()], [right.ring()]]
        });
        let service = PreserveService::load_from_json(
            &json!({
                "type": "FeatureCollection",
                "features": [feature("Multi", geometry)]
            })
            .to_string(),
        )
        .expect("generated MultiPolygon must load");

        let target = if in_left { &left } else { &right };
        let points: Vec<(f64, f64)> = fractions
            .iter()
            .map(|(fx, fy)| {
                (
                    f64::from(target.x0) + f64::from(target.x1 - target.x0) * fx,
                    f64::from(target.y0) + f64::from(target.y1 - target.y0) * fy,
                )
            })
            .collect();

        let result = service.find_intersections(&LineString::from(points));
        prop_assert_eq!(names(&result), vec!["Multi"]);
    }

    /// Loading must preserve geometry, and the cached bounding rectangle must
    /// be the geometry's own bounding rectangle (the prefilter depends on it).
    #[test]
    fn loader_preserves_geometry_and_bbox(rect in rect_strategy()) {
        let service = service_from_rects(std::slice::from_ref(&rect));

        prop_assert_eq!(service.preserves().len(), 1);
        let preserve = &service.preserves()[0];

        prop_assert_eq!(preserve.name.as_str(), "P0");
        prop_assert_eq!(preserve.url.as_str(), "https://example.com/preserve");
        prop_assert_eq!(&preserve.bbox, &preserve.geometry.bounding_rect().expect("polygon has a bbox"));
        prop_assert_eq!(preserve.bbox.min().x, f64::from(rect.x0));
        prop_assert_eq!(preserve.bbox.min().y, f64::from(rect.y0));
        prop_assert_eq!(preserve.bbox.max().x, f64::from(rect.x1));
        prop_assert_eq!(preserve.bbox.max().y, f64::from(rect.y1));
    }

    /// Degenerate activity lines must be handled without panicking.
    #[test]
    fn degenerate_lines_are_safe(rects in prop::collection::vec(rect_strategy(), 0..3)) {
        let service = service_from_rects(&rects);

        let empty: LineString<f64> = LineString::from(Vec::<(f64, f64)>::new());
        prop_assert!(service.find_intersections(&empty).is_empty());
        // A single point is not an area/line intersection, but must not panic.
        let single: LineString<f64> = LineString::from(vec![(0.0, 0.0)]);
        let _ = service.find_intersections(&single);
    }
}

#[test]
fn preserves_without_url_are_skipped() {
    let geometry = polygon_geometry(&Rect {
        x0: 0,
        y0: 0,
        x1: 1,
        y1: 1,
    });
    let mut missing_url = feature("Missing", geometry.clone());
    missing_url["properties"] = json!({ "name": "Missing" });
    let mut empty_url = feature("Empty", geometry);
    empty_url["properties"] = json!({ "name": "Empty", "url": "" });

    let service = PreserveService::load_from_json(
        &json!({ "type": "FeatureCollection", "features": [missing_url, empty_url] }).to_string(),
    )
    .expect("loading must succeed");

    // Closed preserves are skipped rather than reported as visited.
    assert!(service.preserves().is_empty());
}

#[test]
fn unsupported_geometry_is_rejected() {
    let point = feature(
        "Point",
        json!({ "type": "Point", "coordinates": [0.0, 0.0] }),
    );
    let result = PreserveService::load_from_json(
        &json!({ "type": "FeatureCollection", "features": [point] }).to_string(),
    );

    match result {
        Err(PreserveError::UnsupportedGeometry) => {}
        Err(other) => panic!("expected UnsupportedGeometry, got {other:?}"),
        Ok(_) => panic!("Point geometry must be rejected"),
    }
}
