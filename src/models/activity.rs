// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@kernel.org>

//! Strava activity model for storage and API.

use serde::{Deserialize, Serialize};

/// Stored activity record in Firestore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Activity {
    /// Strava activity ID (also used as document ID)
    pub strava_activity_id: u64,
    /// Strava athlete ID (owner)
    pub athlete_id: u64,
    /// Activity name/title
    pub name: String,
    /// Sport type (Ride, Run, Hike, etc.)
    pub sport_type: String,
    /// Start date/time (ISO 8601)
    pub start_date: String,
    /// Distance in meters
    pub distance_meters: f64,
    /// List of preserve names that were visited
    pub preserves_visited: Vec<String>,
    /// Source: "webhook" or "backfill"
    pub source: String,
    /// Device name (e.g. "Garmin Edge 530")
    pub device_name: Option<String>,
    /// Whether the description was annotated
    pub annotation_added: bool,
    /// When this activity was processed
    pub processed_at: String,
}

/// Activity-preserve join record for efficient queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityPreserve {
    /// Strava athlete ID
    pub athlete_id: u64,
    /// Strava activity ID
    pub activity_id: u64,
    /// Preserve name
    pub preserve_name: String,
    /// Activity start date (for sorting)
    pub start_date: String,
    /// Activity name
    pub activity_name: String,
    /// Sport type
    pub sport_type: String,
}
