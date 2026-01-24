// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@kernel.org>

//! Midpen Preserve model and geometry handling.

use geo::{MultiPolygon, Polygon};
use serde::{Deserialize, Serialize};
#[cfg(feature = "binding-generation")]
use ts_rs::TS;

/// A Midpen Open Space Preserve with its boundary geometry.
#[derive(Debug, Clone)]
pub struct Preserve {
    /// Preserve name (e.g., "Rancho San Antonio")
    pub name: String,
    /// URL to the preserve's page on openspace.org
    pub url: String,
    /// Boundary geometry (can be Polygon or MultiPolygon)
    pub geometry: PreserveGeometry,
}

/// Preserve geometry - either a simple polygon or multi-polygon.
#[derive(Debug, Clone)]
pub enum PreserveGeometry {
    Polygon(Polygon<f64>),
    MultiPolygon(MultiPolygon<f64>),
}

impl PreserveGeometry {
    /// Check if a line string intersects this geometry.
    pub fn intersects(&self, line: &geo::LineString<f64>) -> bool {
        use geo::Intersects;
        match self {
            PreserveGeometry::Polygon(p) => line.intersects(p),
            PreserveGeometry::MultiPolygon(mp) => line.intersects(mp),
        }
    }
}

/// Summary of a preserve for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "binding-generation", derive(TS))]
#[cfg_attr(
    feature = "binding-generation",
    ts(export, export_to = "web/src/lib/generated/")
)]
pub struct PreserveSummary {
    pub name: String,
    pub count: u32,
    pub activities: Vec<PreserveActivity>,
}

/// Activity summary within a preserve context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "binding-generation", derive(TS))]
#[cfg_attr(
    feature = "binding-generation",
    ts(export, export_to = "web/src/lib/generated/")
)]
pub struct PreserveActivity {
    #[cfg_attr(feature = "binding-generation", ts(type = "number"))]
    pub id: u64,
    pub date: String,
    pub sport_type: String,
    pub name: String,
}
