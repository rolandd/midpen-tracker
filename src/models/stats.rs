// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! User statistics aggregates for efficient dashboard queries.
//!
//! These aggregates are pre-computed when activities are processed,
//! reducing dashboard Firestore reads from O(activities) to O(1).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::models::Activity;
use crate::time_utils::format_utc_rfc3339;
use chrono::{DateTime, Utc};

/// Pre-computed statistics for a user.
///
/// Stored at: `user_stats/{athlete_id}`
///
/// Updated atomically with activity writes via Firestore transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    // ─── Preserve Stats ──────────────────────────────────────────
    /// Visit count per preserve (for dashboard counts)
    #[serde(default)]
    pub preserves: HashMap<String, u32>,
    /// First visit date per preserve (ISO 8601)
    #[serde(default)]
    pub preserve_first_visit: HashMap<String, String>,
    /// Most recent visit date per preserve (ISO 8601)
    #[serde(default)]
    pub preserve_last_visit: HashMap<String, String>,

    // ─── Activity Stats ──────────────────────────────────────────
    /// Total activities processed
    #[serde(default)]
    pub total_activities: u32,
    /// Total distance across all activities (meters)
    #[serde(default)]
    pub total_distance_meters: f64,

    // ─── By Sport Type ───────────────────────────────────────────
    /// Activity count per sport type (for pie charts)
    #[serde(default)]
    pub activities_by_sport: HashMap<String, u32>,
    /// Total distance per sport type (meters)
    #[serde(default)]
    pub distance_by_sport: HashMap<String, f64>,

    // ─── Time Series ─────────────────────────────────────────────
    /// Activity count per month ("YYYY-MM" format)
    #[serde(default)]
    pub activities_by_month: HashMap<String, u32>,
    /// Activity count per year ("YYYY" format)
    #[serde(default)]
    pub activities_by_year: HashMap<String, u32>,

    // ─── Preserves by Year ───────────────────────────────────────
    /// Visit count per preserve per year: { "2025": { "Rancho": 5 } }
    #[serde(default)]
    pub preserves_by_year: HashMap<String, HashMap<String, u32>>,

    // ─── Idempotency ─────────────────────────────────────────────
    /// Set of processed activity IDs (for duplicate detection)
    #[serde(default)]
    pub processed_activity_ids: HashSet<u64>,

    // ─── Backfill Progress ───────────────────────────────────────
    /// Number of activities queued but not yet processed
    #[serde(default)]
    pub pending_activities: u32,

    // ─── Metadata ────────────────────────────────────────────────
    /// Last update timestamp (ISO 8601)
    #[serde(default)]
    pub updated_at: String,
}

impl Default for UserStats {
    fn default() -> Self {
        Self {
            preserves: HashMap::new(),
            preserve_first_visit: HashMap::new(),
            preserve_last_visit: HashMap::new(),
            total_activities: 0,
            total_distance_meters: 0.0,
            activities_by_sport: HashMap::new(),
            distance_by_sport: HashMap::new(),
            activities_by_month: HashMap::new(),
            activities_by_year: HashMap::new(),
            preserves_by_year: HashMap::new(),
            processed_activity_ids: HashSet::new(),
            pending_activities: 0,
            updated_at: String::new(),
        }
    }
}

impl UserStats {
    /// Update stats with a new activity.
    ///
    /// Returns `true` if the activity was processed (new).
    /// Returns `false` if the activity was already processed (duplicate).
    pub fn update_from_activity(&mut self, activity: &Activity, now: &str) -> bool {
        // Idempotency check: skip if already processed
        if self
            .processed_activity_ids
            .contains(&activity.strava_activity_id)
        {
            return false;
        }

        // Mark as processed
        self.processed_activity_ids
            .insert(activity.strava_activity_id);
        self.updated_at = now.to_string();

        // Update preserve counts and visit dates
        let activity_date = format_utc_rfc3339(activity.start_date);
        for preserve_name in &activity.preserves_visited {
            *self.preserves.entry(preserve_name.clone()).or_insert(0) += 1;

            // Track first visit (update if earlier)
            self.preserve_first_visit
                .entry(preserve_name.clone())
                .and_modify(|first| {
                    if activity_date < *first {
                        *first = activity_date.clone();
                    }
                })
                .or_insert_with(|| activity_date.clone());

            // Track last visit (update if later)
            self.preserve_last_visit
                .entry(preserve_name.clone())
                .and_modify(|last| {
                    if activity_date > *last {
                        *last = activity_date.clone();
                    }
                })
                .or_insert_with(|| activity_date.clone());
        }

        // Update activity totals
        self.total_activities += 1;
        self.total_distance_meters += activity.distance_meters;

        // Update sport type stats
        *self
            .activities_by_sport
            .entry(activity.sport_type.clone())
            .or_insert(0) += 1;
        *self
            .distance_by_sport
            .entry(activity.sport_type.clone())
            .or_insert(0.0) += activity.distance_meters;

        // Update time series (extract YYYY-MM and YYYY from start_date)
        let month_key = extract_month_key(activity.start_date);
        *self.activities_by_month.entry(month_key).or_insert(0) += 1;

        let year_key = extract_year_key(activity.start_date);
        *self.activities_by_year.entry(year_key.clone()).or_insert(0) += 1;

        // Update preserves_by_year for year-filtered queries
        let year_preserves = self.preserves_by_year.entry(year_key).or_default();
        for preserve_name in &activity.preserves_visited {
            *year_preserves.entry(preserve_name.clone()).or_insert(0) += 1;
        }

        true
    }

    /// Check if an activity is the first or last visit for any of the preserves it covers.
    ///
    /// If it is a boundary activity, O(1) decrement is not possible because we wouldn't
    /// know the new first/last visit date without a full recalculation.
    pub fn is_boundary_activity(&self, activity: &Activity) -> bool {
        let activity_date = format_utc_rfc3339(activity.start_date);
        for preserve_name in &activity.preserves_visited {
            if let Some(first) = self.preserve_first_visit.get(preserve_name) {
                if &activity_date == first {
                    return true;
                }
            }
            if let Some(last) = self.preserve_last_visit.get(preserve_name) {
                if &activity_date == last {
                    return true;
                }
            }
        }
        false
    }

    /// Update stats by removing an activity (incremental decrement).
    ///
    /// Returns `true` if the activity was found and removed.
    ///
    /// NOTE: This should only be used if `is_boundary_activity` returns `false`
    /// for this activity, otherwise first/last visit dates will become incorrect.
    pub fn decrement_from_activity(&mut self, activity: &Activity, now: &str) -> bool {
        // Idempotency check: skip if not processed
        if !self
            .processed_activity_ids
            .remove(&activity.strava_activity_id)
        {
            return false;
        }

        self.updated_at = now.to_string();

        // Update preserve counts
        for preserve_name in &activity.preserves_visited {
            if let Some(count) = self.preserves.get_mut(preserve_name) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.preserves.remove(preserve_name);
                    self.preserve_first_visit.remove(preserve_name);
                    self.preserve_last_visit.remove(preserve_name);
                }
            }
        }

        // Update activity totals
        self.total_activities = self.total_activities.saturating_sub(1);
        self.total_distance_meters = (self.total_distance_meters - activity.distance_meters).max(0.0);

        // Update sport type stats
        if let Some(count) = self.activities_by_sport.get_mut(&activity.sport_type) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.activities_by_sport.remove(&activity.sport_type);
            }
        }
        if let Some(dist) = self.distance_by_sport.get_mut(&activity.sport_type) {
            *dist = (*dist - activity.distance_meters).max(0.0);
            if *dist <= 0.0 {
                self.distance_by_sport.remove(&activity.sport_type);
            }
        }

        // Update time series
        let month_key = extract_month_key(activity.start_date);
        if let Some(count) = self.activities_by_month.get_mut(&month_key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.activities_by_month.remove(&month_key);
            }
        }

        let year_key = extract_year_key(activity.start_date);
        if let Some(count) = self.activities_by_year.get_mut(&year_key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.activities_by_year.remove(&year_key);
            }
        }

        // Update preserves_by_year
        if let Some(year_preserves) = self.preserves_by_year.get_mut(&year_key) {
            for preserve_name in &activity.preserves_visited {
                if let Some(count) = year_preserves.get_mut(preserve_name) {
                    *count = count.saturating_sub(1);
                    if *count == 0 {
                        year_preserves.remove(preserve_name);
                    }
                }
            }
            if year_preserves.is_empty() {
                self.preserves_by_year.remove(&year_key);
            }
        }

        true
    }
}

/// Extract "YYYY-MM" from a UTC timestamp.
fn extract_month_key(date: DateTime<Utc>) -> String {
    date.format("%Y-%m").to_string()
}

/// Extract "YYYY" from a UTC timestamp.
fn extract_year_key(date: DateTime<Utc>) -> String {
    date.format("%Y").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_activity(
        id: u64,
        sport: &str,
        date: &str,
        distance: f64,
        preserves: Vec<&str>,
    ) -> Activity {
        let start_date = chrono::DateTime::parse_from_rfc3339(date)
            .unwrap()
            .with_timezone(&chrono::Utc);

        Activity {
            strava_activity_id: id,
            athlete_id: 12345,
            name: format!("Test Activity {}", id),
            sport_type: sport.to_string(),
            start_date,
            distance_meters: distance,
            preserves_visited: preserves.into_iter().map(String::from).collect(),
            source: "test".to_string(),
            annotation_added: false,
            processed_at: "2024-01-15T12:00:00Z".to_string(),
            device_name: None,
        }
    }

    #[test]
    fn test_update_from_activity_basic() {
        let mut stats = UserStats::default();
        let activity = make_activity(
            1,
            "Ride",
            "2024-01-15T10:00:00Z",
            10000.0,
            vec!["Rancho San Antonio"],
        );

        let processed = stats.update_from_activity(&activity, "2024-01-15T12:00:00Z");

        assert!(processed);
        assert_eq!(stats.total_activities, 1);
        assert_eq!(stats.total_distance_meters, 10000.0);
        assert_eq!(stats.preserves.get("Rancho San Antonio"), Some(&1));
        assert_eq!(stats.activities_by_sport.get("Ride"), Some(&1));
        assert_eq!(stats.activities_by_month.get("2024-01"), Some(&1));
        assert_eq!(stats.activities_by_year.get("2024"), Some(&1));
    }

    #[test]
    fn test_idempotency_skips_duplicate() {
        let mut stats = UserStats::default();
        let activity = make_activity(1, "Ride", "2024-01-15T10:00:00Z", 10000.0, vec![]);

        stats.update_from_activity(&activity, "2024-01-15T12:00:00Z");
        let processed_again = stats.update_from_activity(&activity, "2024-01-15T13:00:00Z");

        assert!(!processed_again);
        assert_eq!(stats.total_activities, 1); // Not incremented twice
    }

    #[test]
    fn test_multiple_preserves_per_activity() {
        let mut stats = UserStats::default();
        let activity = make_activity(
            1,
            "Hike",
            "2024-01-15T10:00:00Z",
            5000.0,
            vec!["Preserve A", "Preserve B"],
        );

        stats.update_from_activity(&activity, "2024-01-15T12:00:00Z");

        assert_eq!(stats.preserves.get("Preserve A"), Some(&1));
        assert_eq!(stats.preserves.get("Preserve B"), Some(&1));
        assert_eq!(stats.total_activities, 1); // Only one activity
    }

    #[test]
    fn test_first_last_visit_tracking() {
        let mut stats = UserStats::default();

        let activity1 = make_activity(1, "Ride", "2024-01-10T10:00:00Z", 5000.0, vec!["Rancho"]);
        let activity2 = make_activity(2, "Run", "2024-01-20T10:00:00Z", 3000.0, vec!["Rancho"]);

        stats.update_from_activity(&activity1, "now");
        stats.update_from_activity(&activity2, "now");

        assert_eq!(
            stats.preserve_first_visit.get("Rancho"),
            Some(&"2024-01-10T10:00:00Z".to_string())
        );
        assert_eq!(
            stats.preserve_last_visit.get("Rancho"),
            Some(&"2024-01-20T10:00:00Z".to_string())
        );
        assert_eq!(stats.preserves.get("Rancho"), Some(&2));
    }

    #[test]
    fn test_first_last_visit_out_of_order() {
        let mut stats = UserStats::default();

        // Process a "middle" activity first
        let activity_mid = make_activity(2, "Run", "2024-01-20T10:00:00Z", 3000.0, vec!["Rancho"]);
        stats.update_from_activity(&activity_mid, "now");

        assert_eq!(
            stats.preserve_first_visit.get("Rancho"),
            Some(&"2024-01-20T10:00:00Z".to_string())
        );
        assert_eq!(
            stats.preserve_last_visit.get("Rancho"),
            Some(&"2024-01-20T10:00:00Z".to_string())
        );

        // Process an earlier activity (should update first_visit)
        let activity_early =
            make_activity(1, "Ride", "2024-01-10T10:00:00Z", 5000.0, vec!["Rancho"]);
        stats.update_from_activity(&activity_early, "now");

        assert_eq!(
            stats.preserve_first_visit.get("Rancho"),
            Some(&"2024-01-10T10:00:00Z".to_string())
        );
        assert_eq!(
            stats.preserve_last_visit.get("Rancho"),
            Some(&"2024-01-20T10:00:00Z".to_string())
        );

        // Process a later activity (should update last_visit)
        let activity_late =
            make_activity(3, "Hike", "2024-01-30T10:00:00Z", 2000.0, vec!["Rancho"]);
        stats.update_from_activity(&activity_late, "now");

        assert_eq!(
            stats.preserve_first_visit.get("Rancho"),
            Some(&"2024-01-10T10:00:00Z".to_string())
        );
        assert_eq!(
            stats.preserve_last_visit.get("Rancho"),
            Some(&"2024-01-30T10:00:00Z".to_string())
        );
    }

    #[test]
    fn test_preserves_by_year() {
        let mut stats = UserStats::default();

        // Activities in different years
        let activity_2024 = make_activity(
            1,
            "Ride",
            "2024-06-15T10:00:00Z",
            5000.0,
            vec!["Rancho", "Fremont"],
        );
        let activity_2025 = make_activity(2, "Run", "2025-01-10T10:00:00Z", 3000.0, vec!["Rancho"]);
        let activity_2025b = make_activity(
            3,
            "Hike",
            "2025-03-20T10:00:00Z",
            2000.0,
            vec!["Rancho", "Pulgas"],
        );

        stats.update_from_activity(&activity_2024, "now");
        stats.update_from_activity(&activity_2025, "now");
        stats.update_from_activity(&activity_2025b, "now");

        // Check 2024 preserves
        let year_2024 = stats.preserves_by_year.get("2024").unwrap();
        assert_eq!(year_2024.get("Rancho"), Some(&1));
        assert_eq!(year_2024.get("Fremont"), Some(&1));
        assert_eq!(year_2024.get("Pulgas"), None);

        // Check 2025 preserves
        let year_2025 = stats.preserves_by_year.get("2025").unwrap();
        assert_eq!(year_2025.get("Rancho"), Some(&2)); // Two activities in 2025
        assert_eq!(year_2025.get("Pulgas"), Some(&1));
        assert_eq!(year_2025.get("Fremont"), None);

        // Total preserves (across all years) still works
        assert_eq!(stats.preserves.get("Rancho"), Some(&3));
    }

    #[test]
    fn test_decrement_from_activity() {
        let mut stats = UserStats::default();
        let activity = make_activity(1, "Ride", "2024-01-15T10:00:00Z", 10000.0, vec!["Rancho"]);

        stats.update_from_activity(&activity, "now");
        assert_eq!(stats.total_activities, 1);

        let removed = stats.decrement_from_activity(&activity, "later");
        assert!(removed);
        assert_eq!(stats.total_activities, 0);
        assert_eq!(stats.total_distance_meters, 0.0);
        assert!(stats.preserves.is_empty());
        assert!(stats.processed_activity_ids.is_empty());
        assert!(stats.activities_by_sport.is_empty());
        assert!(stats.activities_by_month.is_empty());
        assert!(stats.activities_by_year.is_empty());
        assert!(stats.preserves_by_year.is_empty());
    }

    #[test]
    fn test_is_boundary_activity() {
        let mut stats = UserStats::default();

        let activity1 = make_activity(1, "Ride", "2024-01-10T10:00:00Z", 5000.0, vec!["Rancho"]);
        let activity2 = make_activity(2, "Ride", "2024-01-15T10:00:00Z", 5000.0, vec!["Rancho"]);
        let activity3 = make_activity(3, "Ride", "2024-01-20T10:00:00Z", 5000.0, vec!["Rancho"]);

        stats.update_from_activity(&activity1, "now");
        stats.update_from_activity(&activity2, "now");
        stats.update_from_activity(&activity3, "now");

        // activity1 is first visit
        assert!(stats.is_boundary_activity(&activity1));
        // activity3 is last visit
        assert!(stats.is_boundary_activity(&activity3));
        // activity2 is neither
        assert!(!stats.is_boundary_activity(&activity2));
    }
}
