//! User statistics aggregates for efficient dashboard queries.
//!
//! These aggregates are pre-computed when activities are processed,
//! reducing dashboard Firestore reads from O(activities) to O(1).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::models::Activity;

/// Pre-computed statistics for a user.
///
/// Stored at: `users/{athlete_id}/stats/aggregates`
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
        for preserve_name in &activity.preserves_visited {
            *self.preserves.entry(preserve_name.clone()).or_insert(0) += 1;

            // Track first visit
            self.preserve_first_visit
                .entry(preserve_name.clone())
                .or_insert_with(|| activity.start_date.clone());

            // Always update last visit (assuming activities processed in order)
            self.preserve_last_visit
                .insert(preserve_name.clone(), activity.start_date.clone());
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
        if let Some(month_key) = extract_month_key(&activity.start_date) {
            *self.activities_by_month.entry(month_key).or_insert(0) += 1;
        }
        if let Some(year_key) = extract_year_key(&activity.start_date) {
            *self.activities_by_year.entry(year_key).or_insert(0) += 1;
        }

        true
    }
}

/// Extract "YYYY-MM" from an ISO 8601 date string.
fn extract_month_key(date: &str) -> Option<String> {
    // ISO 8601: "2024-01-15T10:30:00Z" -> "2024-01"
    if date.len() >= 7 {
        Some(date[..7].to_string())
    } else {
        None
    }
}

/// Extract "YYYY" from an ISO 8601 date string.
fn extract_year_key(date: &str) -> Option<String> {
    if date.len() >= 4 {
        Some(date[..4].to_string())
    } else {
        None
    }
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
        Activity {
            strava_activity_id: id,
            athlete_id: 12345,
            name: format!("Test Activity {}", id),
            sport_type: sport.to_string(),
            start_date: date.to_string(),
            distance_meters: distance,
            preserves_visited: preserves.into_iter().map(String::from).collect(),
            source: "test".to_string(),
            annotation_added: false,
            processed_at: "2024-01-15T12:00:00Z".to_string(),
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
}
