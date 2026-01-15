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
use crate::models::Activity;
use crate::services::strava::StravaClient;
use crate::services::{KmsService, PreserveService};

/// Process an activity and detect preserve intersections.
pub struct ActivityProcessor {
    strava: StravaClient,
    preserves: PreserveService,
    db: FirestoreDb,
    kms: KmsService,
}

impl ActivityProcessor {
    pub fn new(
        strava: StravaClient,
        preserves: PreserveService,
        db: FirestoreDb,
        kms: KmsService,
    ) -> Self {
        Self {
            strava,
            preserves,
            db,
            kms,
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

        // 1. Get user tokens
        let tokens = self
            .db
            .get_tokens(athlete_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Tokens for athlete {}", athlete_id)))?;

        // 2. Decrypt tokens
        let (mut access_token, refresh_token) = crate::services::kms::decrypt_tokens(
            &self.kms,
            &tokens.access_token_encrypted,
            &tokens.refresh_token_encrypted,
        )
        .await?;

        // Check if token is expired (or close to expiring)
        let expires_at = chrono::DateTime::parse_from_rfc3339(&tokens.expires_at)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to parse expiry: {}", e)))?
            .with_timezone(&chrono::Utc);

        // Refresh if expiring within 5 minutes
        if chrono::Utc::now() + chrono::Duration::minutes(5) >= expires_at {
            tracing::info!(athlete_id, "Access token expired, refreshing");

            let new_tokens = self.strava.refresh_token(&refresh_token).await?;

            // Re-encrypt new tokens
            let (new_enc_access, new_enc_refresh) = crate::services::kms::encrypt_tokens(
                &self.kms,
                &new_tokens.access_token,
                &new_tokens.refresh_token,
            )
            .await?;

            // Update in DB
            let mut updated_tokens = tokens.clone();
            updated_tokens.access_token_encrypted = new_enc_access;
            updated_tokens.refresh_token_encrypted = new_enc_refresh;
            updated_tokens.expires_at = chrono::DateTime::from_timestamp(new_tokens.expires_at, 0)
                .unwrap_or_default()
                .to_rfc3339();

            self.db.set_tokens(athlete_id, &updated_tokens).await?;

            // Use new token for request
            access_token = new_tokens.access_token;
        }

        // 3. Fetch activity from Strava
        let strava_activity = self.strava.get_activity(&access_token, activity_id).await?;

        // 4. Get polyline and detect preserves
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

        // 5. Build annotation if any preserves
        let annotation_added = if !preserves_visited.is_empty() && source == "webhook" {
            let annotation = build_annotation(&preserves_visited);
            let new_description =
                append_annotation(strava_activity.description.as_deref(), &annotation);

            // Update activity description on Strava
            self.strava
                .update_activity_description(&access_token, activity_id, &new_description)
                .await?;
            true
        } else {
            false
        };

        // 6. Store activity and update stats
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

        // 7. Update user stats aggregate (idempotent)
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
    let preserve_list = preserves.join(", ");
    format!("🌲 Midpen Preserves: {}", preserve_list)
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
