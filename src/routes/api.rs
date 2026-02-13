// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! API routes for authenticated users.

use crate::db::firestore::ActivityQueryCursor;
use crate::error::Result;
use crate::middleware::auth::AuthUser;
use crate::models::preserve::PreserveSummary;
use crate::models::ActivityPreserve;
use crate::services::tasks::DeleteUserPayload;
use crate::time_utils::format_utc_rfc3339;
use crate::AppState;
use axum::{
    extract::{Query, State},
    routing::{delete, get},
    Extension, Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[cfg(feature = "binding-generation")]
use ts_rs::TS;

const STUCK_PENDING_COUNT_TIMEOUT_MINUTES: i64 = 15;

/// API routes (require authentication via JWT).
/// The auth middleware is applied in routes/mod.rs for these routes.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/me", get(get_me))
        .route("/api/activities", get(get_activities))
        .route("/api/stats/preserves", get(get_preserve_stats))
        .route("/api/account", delete(delete_account))
}

// ─── User Profile ────────────────────────────────────────────

/// Current user response.
#[derive(Serialize)]
#[cfg_attr(feature = "binding-generation", derive(TS))]
#[cfg_attr(
    feature = "binding-generation",
    ts(export, export_to = "web/src/lib/generated/")
)]
pub struct UserResponse {
    #[cfg_attr(feature = "binding-generation", ts(type = "number"))]
    pub athlete_id: u64,
    pub firstname: String,
    pub lastname: String,
    pub profile_picture: Option<String>,
    pub deletion_requested_at: Option<String>,
}

/// Get current user profile.
async fn get_me(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<UserResponse>> {
    let user_profile = state.db.get_user(user.athlete_id).await?.ok_or_else(|| {
        crate::error::AppError::NotFound(format!("User {} not found", user.athlete_id))
    })?;

    Ok(Json(UserResponse {
        athlete_id: user_profile.strava_athlete_id,
        firstname: user_profile.firstname,
        lastname: user_profile.lastname,
        profile_picture: user_profile.profile_picture,
        deletion_requested_at: user_profile.deletion_requested_at,
    }))
}

// ─── Account Deletion ────────────────────────────────────────

/// Response for account deletion.
#[derive(Serialize)]
#[cfg_attr(feature = "binding-generation", derive(TS))]
#[cfg_attr(
    feature = "binding-generation",
    ts(export, export_to = "web/src/lib/generated/")
)]
pub struct DeleteAccountResponse {
    pub success: bool,
    pub message: String,
}

/// Delete user's account and all associated data (GDPR compliance).
///
/// This queues a deletion task and returns immediately.
/// The task will:
/// 1. Delete tokens from DB (blocks concurrent activity processing)
/// 2. Delete all user data from Firestore
/// 3. Deauthorize with Strava
async fn delete_account(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<DeleteAccountResponse>> {
    tracing::info!(
        athlete_id = user.athlete_id,
        "User-initiated account deletion"
    );

    // Mark user as pending deletion (for UI feedback)
    // We fetch-modify-write to preserve other fields
    if let Some(mut user_profile) = state.db.get_user(user.athlete_id).await? {
        user_profile.deletion_requested_at = Some(chrono::Utc::now().to_rfc3339());
        state.db.upsert_user(&user_profile).await?;
    } else {
        // User already gone? Rare but possible. Proceed to queue task just in case tokens remain.
        tracing::warn!(
            athlete_id = user.athlete_id,
            "User profile not found during deletion request"
        );
    }

    // Queue deletion task
    let payload = DeleteUserPayload {
        athlete_id: user.athlete_id,
        source: "user_request".to_string(),
    };

    state
        .tasks_service
        .queue_delete_user(&state.config.api_url, payload)
        .await?;

    Ok(Json(DeleteAccountResponse {
        success: true,
        message: "Account deletion initiated. All data will be removed.".to_string(),
    }))
}

// ─── Activities ──────────────────────────────────────────────

#[derive(Deserialize)]
struct ActivitiesQuery {
    /// Filter by preserve name
    preserve: Option<String>,
    /// Filter by start date (ISO 8601)
    after: Option<String>,
    /// Cursor for forward pagination (opaque token).
    cursor: Option<String>,
    /// Pagination: page number (1-indexed)
    #[serde(default = "default_page")]
    page: u32,
    /// Pagination: items per page
    #[serde(default = "default_per_page")]
    per_page: u32,
}

fn default_page() -> u32 {
    1
}
fn default_per_page() -> u32 {
    50
}

const MAX_PER_PAGE: u32 = 100;
const CURSOR_PARTS: usize = 3;

fn parse_after_timestamp(after: Option<&str>) -> Result<Option<chrono::DateTime<chrono::Utc>>> {
    after
        .map(|raw| {
            chrono::DateTime::parse_from_rfc3339(raw)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|_| {
                    crate::error::AppError::BadRequest(
                        "Invalid 'after' parameter: must be RFC3339 datetime".to_string(),
                    )
                })
        })
        .transpose()
}

fn parse_cursor(cursor: Option<&str>) -> Result<Option<ActivityQueryCursor>> {
    cursor
        .map(|raw| {
            let invalid_cursor =
                || crate::error::AppError::BadRequest("Invalid 'cursor' parameter".to_string());

            let decoded = URL_SAFE_NO_PAD.decode(raw).map_err(|_| invalid_cursor())?;
            let decoded_str = std::str::from_utf8(&decoded).map_err(|_| invalid_cursor())?;

            let parts: Vec<&str> = decoded_str.split(':').collect();
            if parts.len() != CURSOR_PARTS {
                return Err(invalid_cursor());
            }

            let seconds = parts[0].parse::<i64>().map_err(|_| invalid_cursor())?;
            let nanos = parts[1].parse::<u32>().map_err(|_| invalid_cursor())?;
            let activity_id = parts[2].parse::<u64>().map_err(|_| invalid_cursor())?;
            let start_date =
                chrono::DateTime::from_timestamp(seconds, nanos).ok_or_else(invalid_cursor)?;

            Ok(ActivityQueryCursor {
                start_date,
                activity_id,
            })
        })
        .transpose()
}

fn encode_cursor(cursor: ActivityQueryCursor) -> String {
    let payload = format!(
        "{}:{}:{}",
        cursor.start_date.timestamp(),
        cursor.start_date.timestamp_subsec_nanos(),
        cursor.activity_id
    );
    URL_SAFE_NO_PAD.encode(payload)
}

#[derive(Serialize)]
#[cfg_attr(feature = "binding-generation", derive(TS))]
#[cfg_attr(
    feature = "binding-generation",
    ts(export, export_to = "web/src/lib/generated/")
)]
pub struct ActivitiesResponse {
    pub activities: Vec<ActivitySummary>,
    pub page: u32,
    pub per_page: u32,
    /// Total number of activities matching the query.
    /// For cursor-based pagination, this is 0 if `next_cursor` is present,
    /// as the exact total is not known.
    pub total: u32,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[cfg_attr(feature = "binding-generation", derive(TS))]
#[cfg_attr(
    feature = "binding-generation",
    ts(export, export_to = "web/src/lib/generated/")
)]
pub struct ActivitySummary {
    #[cfg_attr(feature = "binding-generation", ts(type = "number"))]
    pub id: u64,
    pub name: String,
    pub sport_type: String,
    pub start_date: String,
    pub preserves: Vec<String>,
}

/// Get user's activities with optional filtering.
async fn get_activities(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
    Query(params): Query<ActivitiesQuery>,
) -> Result<Json<ActivitiesResponse>> {
    tracing::debug!(
        athlete_id = user.athlete_id,
        preserve = ?params.preserve,
        after = ?params.after,
        cursor = ?params.cursor,
        page = params.page,
        "Fetching activities"
    );

    let limit = params.per_page.min(MAX_PER_PAGE);
    let after_timestamp = parse_after_timestamp(params.after.as_deref())?;
    let cursor = parse_cursor(params.cursor.as_deref())?;

    if params.page < 1 {
        return Err(crate::error::AppError::BadRequest(
            "Page must be greater than 0".to_string(),
        ));
    }

    if params.preserve.is_none() && params.page != 1 {
        return Err(crate::error::AppError::BadRequest(
            "Page-based pagination is no longer supported for this query; use 'cursor'".to_string(),
        ));
    }

    if params.preserve.is_some() && cursor.is_some() {
        return Err(crate::error::AppError::BadRequest(
            "'cursor' is not supported when filtering by preserve".to_string(),
        ));
    }

    let (activities, total, next_cursor) = if let Some(preserve_name) = params.preserve {
        // Query by preserve using the join collection
        let results: Vec<ActivityPreserve> = state
            .db
            .get_activities_for_preserve(user.athlete_id, &preserve_name, after_timestamp)
            .await?;

        // Map to summaries
        let summaries: Vec<ActivitySummary> = results
            .into_iter()
            .map(|r| ActivitySummary {
                id: r.activity_id,
                name: r.activity_name,
                sport_type: r.sport_type,
                start_date: format_utc_rfc3339(r.start_date),
                preserves: vec![r.preserve_name],
            })
            .collect();

        let total_count = summaries.len() as u32;

        // Pagination (simple in-memory for now since these lists are small per preserve)
        // Use checked multiplication to prevent overflow and cast to usize safely
        let start = (params.page as usize - 1)
            .checked_mul(limit as usize)
            .ok_or_else(|| {
                crate::error::AppError::BadRequest("Page number causes overflow".to_string())
            })?;

        let paged_activities = if start < summaries.len() {
            let end = start.saturating_add(limit as usize).min(summaries.len());
            summaries[start..end].to_vec()
        } else {
            vec![]
        };

        (paged_activities, total_count, None)
    } else {
        // Cursor-based pagination for user activity listing.
        // Fetch one extra item to determine if another page is available.
        let fetch_limit = limit.saturating_add(1);
        let mut results = state
            .db
            .get_activities_for_user(user.athlete_id, after_timestamp, cursor, fetch_limit)
            .await?;

        let has_more = results.len() > limit as usize;
        if has_more {
            results.truncate(limit as usize);
        }

        let next_cursor = if has_more {
            results.last().map(|a| {
                encode_cursor(ActivityQueryCursor {
                    start_date: a.start_date,
                    activity_id: a.strava_activity_id,
                })
            })
        } else {
            None
        };

        let summaries: Vec<ActivitySummary> = results
            .into_iter()
            .map(|a| ActivitySummary {
                id: a.strava_activity_id,
                name: a.name,
                sport_type: a.sport_type,
                start_date: format_utc_rfc3339(a.start_date),
                preserves: a.preserves_visited,
            })
            .collect();

        // Cursor pagination doesn't provide a cheap exact total.
        // We return 0 if there are more results to indicate the total is unknown.
        let total = if next_cursor.is_some() {
            0
        } else {
            summaries.len() as u32
        };
        (summaries, total, next_cursor)
    };

    Ok(Json(ActivitiesResponse {
        total,
        activities,
        page: params.page,
        per_page: limit,
        next_cursor,
    }))
}

// ─── Preserve Stats ──────────────────────────────────────────

#[derive(Deserialize)]
struct PreserveStatsQuery {
    /// Include preserves with 0 visits
    #[serde(default)]
    show_unvisited: bool,
}

/// Preserve stats response.
#[derive(Serialize)]
#[cfg_attr(feature = "binding-generation", derive(TS))]
#[cfg_attr(
    feature = "binding-generation",
    ts(export, export_to = "web/src/lib/generated/")
)]
pub struct PreserveStatsResponse {
    /// All-time preserve visit counts
    pub preserves: Vec<PreserveSummary>,
    /// Preserve visits broken down by year: { "2025": { "Rancho": 5 } }
    pub preserves_by_year:
        std::collections::HashMap<String, std::collections::HashMap<String, u32>>,
    pub total_preserves_visited: u32,
    pub total_preserves: u32,
    /// Number of activities still being processed in backfill
    pub pending_activities: u32,
    /// Available years for filtering (sorted descending)
    pub available_years: Vec<String>,
}

/// Get preserve visit stats for current user.
///
/// Uses pre-computed aggregates from `user_stats` collection (1 read).
/// Returns both all-time and per-year preserve counts for frontend filtering.
async fn get_preserve_stats(
    State(state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
    Query(params): Query<PreserveStatsQuery>,
) -> Result<Json<PreserveStatsResponse>> {
    let all_preserves = state.preserve_service.preserves();
    let total_preserves = all_preserves.len() as u32;

    tracing::debug!(
        athlete_id = user.athlete_id,
        show_unvisited = params.show_unvisited,
        total_preserves,
        "Fetching preserve stats"
    );

    // Fetch user stats aggregate (1 Firestore read)
    let stats = state
        .db
        .get_user_stats(user.athlete_id)
        .await?
        .unwrap_or_default();

    // Self-Healing: If activities are still "pending" after 15 minutes of inactivity,
    // assume the tasks were lost or dropped and reset the count to 0.
    // This fixes "stuck" counters for users without requiring a re-login.
    let mut pending_activities = stats.pending_activities;
    if pending_activities > 0 {
        if let Ok(updated_at) = chrono::DateTime::parse_from_rfc3339(&stats.updated_at) {
            let now = chrono::Utc::now();
            let elapsed = now.signed_duration_since(updated_at.with_timezone(&chrono::Utc));

            if elapsed.num_minutes() >= STUCK_PENDING_COUNT_TIMEOUT_MINUTES {
                tracing::info!(
                    athlete_id = user.athlete_id,
                    pending = pending_activities,
                    last_update = %stats.updated_at,
                    elapsed_mins = elapsed.num_minutes(),
                    "Self-healing: Resetting stuck pending count after inactivity"
                );

                // Reset in background to avoid delaying the response
                let state_clone = state.clone();
                let athlete_id = user.athlete_id;
                tokio::spawn(async move {
                    if let Err(e) = state_clone.db.reset_pending_count(athlete_id).await {
                        tracing::warn!(error = %e, "Failed to reset stuck pending count in background");
                    }
                });

                // Return 0 to the user immediately for better UX
                pending_activities = 0;
            }
        }
    }

    // Get available years (sorted descending - most recent first)
    let mut available_years: Vec<String> = stats.preserves_by_year.keys().cloned().collect();
    available_years.sort_by(|a, b| b.cmp(a));

    // Build all-time preserve summaries
    let mut preserves: Vec<PreserveSummary> = if params.show_unvisited {
        all_preserves
            .iter()
            .map(|p| {
                let count = stats.preserves.get(&p.name).copied().unwrap_or(0);
                PreserveSummary {
                    name: p.name.clone(),
                    count,
                    activities: vec![],
                }
            })
            .collect()
    } else {
        stats
            .preserves
            .iter()
            .map(|(name, &count)| PreserveSummary {
                name: name.clone(),
                count,
                activities: vec![],
            })
            .collect()
    };

    preserves.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    let visited_count = preserves.iter().filter(|p| p.count > 0).count() as u32;

    Ok(Json(PreserveStatsResponse {
        preserves,
        preserves_by_year: stats.preserves_by_year,
        total_preserves_visited: visited_count,
        total_preserves,
        pending_activities,
        available_years,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_round_trip() {
        let cursor = ActivityQueryCursor {
            start_date: chrono::DateTime::from_timestamp(1_704_103_200, 123).unwrap(),
            activity_id: 42,
        };

        let encoded = encode_cursor(cursor);
        let decoded = parse_cursor(Some(&encoded)).unwrap().unwrap();

        assert_eq!(decoded.start_date, cursor.start_date);
        assert_eq!(decoded.activity_id, cursor.activity_id);
    }

    #[test]
    fn test_cursor_rejects_invalid_input() {
        let err = parse_cursor(Some("not-base64")).unwrap_err();
        assert!(matches!(err, crate::error::AppError::BadRequest(_)));
    }
}
