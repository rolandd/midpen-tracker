// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Preserve loading and intersection detection service.

use crate::models::preserve::{Preserve, PreserveGeometry};
use geo::{LineString, MultiPolygon, Polygon};
use geojson::GeoJson;
use std::fs;
use std::path::Path;

/// Service for loading preserves and checking activity intersections.
#[derive(Default, Clone)]
pub struct PreserveService {
    preserves: Vec<Preserve>,
}

impl PreserveService {
    /// Load preserves from a GeoJSON file.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, PreserveError> {
        let json_data =
            fs::read_to_string(path.as_ref()).map_err(|e| PreserveError::IoError(e.to_string()))?;
        Self::load_from_json(&json_data)
    }

    /// Load preserves from a GeoJSON string.
    pub fn load_from_json(json_data: &str) -> Result<Self, PreserveError> {
        let geojson: GeoJson = json_data
            .parse()
            .map_err(|e: geojson::Error| PreserveError::ParseError(e.to_string()))?;

        let mut preserves = Vec::new();

        if let GeoJson::FeatureCollection(collection) = geojson {
            for feature in collection.features {
                let name = feature
                    .property("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let url = feature
                    .property("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Skip preserves with no URL (closed to public)
                if url.is_empty() {
                    continue;
                }

                if let Some(geom) = feature.geometry {
                    let geometry = Self::convert_geometry(geom.value)?;
                    preserves.push(Preserve {
                        name,
                        url,
                        geometry,
                    });
                }
            }
        }

        tracing::info!(count = preserves.len(), "Loaded preserves");
        Ok(Self { preserves })
    }

    /// Convert GeoJSON geometry to our internal format.
    fn convert_geometry(value: geojson::Value) -> Result<PreserveGeometry, PreserveError> {
        use std::convert::TryInto;

        // Try as Polygon first
        let poly_result: Result<Polygon<f64>, _> = value.clone().try_into();
        if let Ok(poly) = poly_result {
            return Ok(PreserveGeometry::Polygon(poly));
        }

        // Try as MultiPolygon
        let multi_result: Result<MultiPolygon<f64>, _> = value.try_into();
        if let Ok(multi) = multi_result {
            return Ok(PreserveGeometry::MultiPolygon(multi));
        }

        Err(PreserveError::UnsupportedGeometry)
    }

    /// Get the list of preserves.
    pub fn preserves(&self) -> &[Preserve] {
        &self.preserves
    }

    /// Find all preserves that intersect with a given line string.
    pub fn find_intersections(&self, line: &LineString<f64>) -> Vec<String> {
        self.preserves
            .iter()
            .filter(|p| p.geometry.intersects(line))
            .map(|p| p.name.clone())
            .collect()
    }

    /// Check intersections from an encoded polyline (Strava format, precision 5).
    pub fn find_intersections_from_polyline(
        &self,
        encoded: &str,
    ) -> Result<Vec<String>, PreserveError> {
        let line = polyline::decode_polyline(encoded, 5)
            .map_err(|e| PreserveError::PolylineError(e.to_string()))?;
        Ok(self.find_intersections(&line))
    }
}

/// Errors from preserve operations.
#[derive(Debug, thiserror::Error)]
pub enum PreserveError {
    #[error("Failed to read file: {0}")]
    IoError(String),

    #[error("Failed to parse GeoJSON: {0}")]
    ParseError(String),

    #[error("Unsupported geometry type (expected Polygon or MultiPolygon)")]
    UnsupportedGeometry,

    #[error("Failed to decode polyline: {0}")]
    PolylineError(String),
}
