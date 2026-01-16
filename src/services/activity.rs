//! Activity processing service.
//!
//! Handles the core workflow:
//! 1. Fetch activity from Strava
//! 2. Decode polyline and detect preserves
//! 3. Update activity description (if webhook source)
//! 4. Store results in Firestore
//! 5. Update user stats aggregate

use crate::db::FirestoreDb;
use crate::error::{AppError, Result};
use crate::models::{Activity, ActivityPreserve};
use crate::services::{PreserveService, StravaService};

/// Marker used to detect if an activity has already been annotated.
const ANNOTATION_MARKER: &str = "🌲 Midpen Preserves:";

/// Process an activity and detect preserve intersections.
pub struct ActivityProcessor {
    strava: StravaService,
    preserves: PreserveService,
    db: FirestoreDb,
}

impl ActivityProcessor {
    pub fn new(strava: StravaService, preserves: PreserveService, db: FirestoreDb) -> Self {
        Self {
            strava,
            preserves,
            db,
        }
    }

    /// Process an activity by ID.
    ///
    /// Args:
    /// - athlete_id: Strava athlete ID
    /// - activity_id: Strava activity ID  
    /// - source: "webhook" or "backfill"
    pub async fn process_activity(
        &self,
        athlete_id: u64,
        activity_id: u64,
        source: &str,
    ) -> Result<ProcessResult> {
        tracing::info!(athlete_id, activity_id, source, "Processing activity");

        // 1. Fetch activity from Strava (token management is handled by StravaService)
        let strava_activity = self.strava.get_activity(athlete_id, activity_id).await?;

        // 2. Get polyline and detect preserves
        let polyline = strava_activity
            .get_polyline()
            .ok_or_else(|| AppError::BadRequest("Activity has no polyline".to_string()))?;

        let preserves_visited = self
            .preserves
            .find_intersections_from_polyline(polyline)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Polyline error: {}", e)))?;

        tracing::info!(
            activity_id,
            preserves = ?preserves_visited,
            "Detected preserves"
        );

        // 3. Build annotation if any preserves (webhooks only, and not already annotated)
        let already_annotated = strava_activity
            .description
            .as_deref()
            .map_or(false, |d| d.contains(ANNOTATION_MARKER));

        let annotation_added =
            if !preserves_visited.is_empty() && source == "webhook" && !already_annotated {
                let annotation = build_annotation(&preserves_visited);
                let new_description =
                    append_annotation(strava_activity.description.as_deref(), &annotation);

                // Update activity description on Strava
                self.strava
                    .update_activity_description(athlete_id, activity_id, &new_description)
                    .await?;
                true
            } else {
                false
            };

        // 4. Store activity and update stats
        let now = chrono_now_iso();
        let activity = Activity {
            strava_activity_id: activity_id,
            athlete_id,
            name: strava_activity.name,
            sport_type: strava_activity.sport_type,
            start_date: strava_activity.start_date,
            distance_meters: strava_activity.distance,
            preserves_visited: preserves_visited.clone(),
            source: source.to_string(),
            annotation_added,
            processed_at: now.clone(),
        };

        // Store activity
        self.db.set_activity(&activity).await?;

        // 4b. Store activity-preserve join records
        if !activity.preserves_visited.is_empty() {
            let join_records: Vec<ActivityPreserve> = activity
                .preserves_visited
                .iter()
                .map(|p_name| ActivityPreserve {
                    athlete_id,
                    activity_id,
                    preserve_name: p_name.clone(),
                    start_date: activity.start_date.clone(),
                    activity_name: activity.name.clone(),
                    sport_type: activity.sport_type.clone(),
                })
                .collect();

            if let Err(e) = self.db.batch_set_activity_preserves(&join_records).await {
                tracing::error!(
                    athlete_id,
                    activity_id,
                    error = %e,
                    "Failed to store activity-preserve records"
                );
                // Non-fatal, but search won't work for this activity
            }
        }

        // 5. Update user stats aggregate (idempotent)
        let mut stats = self
            .db
            .get_user_stats(athlete_id)
            .await?
            .unwrap_or_default();

        let was_new = stats.update_from_activity(&activity, &now);
        if was_new {
            self.db.set_user_stats(athlete_id, &stats).await?;
            tracing::info!(
                athlete_id,
                activity_id,
                total_activities = stats.total_activities,
                "Updated user stats"
            );
        } else {
            tracing::debug!(
                athlete_id,
                activity_id,
                "Activity already processed, stats not updated"
            );
        }

        Ok(ProcessResult {
            activity_id,
            preserves_visited,
            annotation_added,
        })
    }
}

/// Result of processing an activity.
#[derive(Debug)]
pub struct ProcessResult {
    pub activity_id: u64,
    pub preserves_visited: Vec<String>,
    pub annotation_added: bool,
}

/// Build the annotation text for preserve visits.
fn build_annotation(preserves: &[String]) -> String {
    let preserve_lines: Vec<String> = preserves.iter().map(|p| format!("  {}", p)).collect();
    format!("{}\n{}", ANNOTATION_MARKER, preserve_lines.join("\n"))
}

/// Append annotation to existing description.
fn append_annotation(existing: Option<&str>, annotation: &str) -> String {
    match existing {
        Some(desc) if !desc.is_empty() => format!("{}\n\n{}", desc, annotation),
        _ => annotation.to_string(),
    }
}

/// Get current time as ISO 8601 string.
fn chrono_now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", secs) // Simplified - could use chrono crate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_annotation_single_preserve() {
        let preserves = vec!["Rancho San Antonio".to_string()];
        let result = build_annotation(&preserves);
        assert_eq!(result, "🌲 Midpen Preserves:\n  Rancho San Antonio");
    }

    #[test]
    fn test_build_annotation_multiple_preserves() {
        let preserves = vec!["Rancho San Antonio".to_string(), "Long Ridge".to_string()];
        let result = build_annotation(&preserves);
        assert_eq!(
            result,
            "🌲 Midpen Preserves:\n  Rancho San Antonio\n  Long Ridge"
        );
    }

    #[test]
    fn test_append_annotation_to_none() {
        let annotation = "🌲 Midpen Preserves: Rancho";
        let result = append_annotation(None, annotation);
        assert_eq!(result, annotation);
    }

    #[test]
    fn test_append_annotation_to_empty_string() {
        let annotation = "🌲 Midpen Preserves: Rancho";
        let result = append_annotation(Some(""), annotation);
        assert_eq!(result, annotation);
    }

    #[test]
    fn test_append_annotation_to_existing_description() {
        let existing = "Great ride today!";
        let annotation = "🌲 Midpen Preserves:\n  Rancho";
        let result = append_annotation(Some(existing), annotation);
        assert_eq!(
            result,
            "Great ride today!\n\n🌲 Midpen Preserves:\n  Rancho"
        );
    }

    #[test]
    fn test_append_annotation_preserves_multiline_description() {
        let existing = "Great ride!\nPerfect weather.";
        let annotation = "🌲 Midpen Preserves:\n  Rancho";
        let result = append_annotation(Some(existing), annotation);
        assert_eq!(
            result,
            "Great ride!\nPerfect weather.\n\n🌲 Midpen Preserves:\n  Rancho"
        );
    }

    #[test]
    fn test_annotation_marker_detection() {
        let annotated = "My ride\n\n🌲 Midpen Preserves:\n  Rancho";
        assert!(annotated.contains(ANNOTATION_MARKER));

        let not_annotated = "Just a normal ride";
        assert!(!not_annotated.contains(ANNOTATION_MARKER));
    }
}
