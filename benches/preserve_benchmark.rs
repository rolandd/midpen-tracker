use criterion::{black_box, criterion_group, criterion_main, Criterion};
use geo::LineString;
use midpen_tracker::services::PreserveService;
use serde_json::Value;
use std::fs;

fn benchmark_find_intersections(c: &mut Criterion) {
    // Load the service once
    let service = PreserveService::load_from_file("data/midpen_boundaries.geojson")
        .expect("Failed to load preserves");

    // Load the realistic activity fixture
    let fixture_content = fs::read_to_string("tests/fixtures/activity_16906743520.json")
        .expect("Failed to read fixture");
    let fixture_json: Value =
        serde_json::from_str(&fixture_content).expect("Failed to parse fixture");

    let polyline_str = fixture_json["map"]["summary_polyline"]
        .as_str()
        .expect("Failed to find summary_polyline");

    // Decode the real polyline (intersects with preserves)
    let real_line = polyline::decode_polyline(polyline_str, 5).expect("Failed to decode polyline");

    // Create a shifted version that is complex but far away (Nevada)
    // Shift by adding 5 degrees to longitude (moves east)
    let shifted_line_coords: Vec<_> = real_line.0.iter().map(|c| (c.x + 5.0, c.y)).collect();
    let shifted_line = LineString::from(shifted_line_coords);

    let mut group = c.benchmark_group("complex_intersections");

    group.bench_function("real_activity_intersects", |b| {
        b.iter(|| service.find_intersections(black_box(&real_line)))
    });

    group.bench_function("shifted_activity_far_away", |b| {
        b.iter(|| service.find_intersections(black_box(&shifted_line)))
    });

    group.finish();
}

criterion_group!(benches, benchmark_find_intersections);
criterion_main!(benches);
