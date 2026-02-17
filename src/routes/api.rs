// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! API routes for authenticated users.

use crate::error::Result;
use crate::middleware::auth::AuthUser;
use crate::models::preserve::PreserveSummary;
use crate::models::ActivityPreserve;
use crate::services::tasks::DeleteUserPayload;
use crate::AppState;
use axum::{
    extract::{Query, State},
    routing::{delete, get},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[cfg(feature = "binding-generation")]
use ts_rs::TS;

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
    pub total: u32,
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
        page = params.page,
        "Fetching activities"
    );

    let limit = params.per_page.min(MAX_PER_PAGE);

    if params.page < 1 {
        return Err(crate::error::AppError::BadRequest(
            "Page must be greater than 0".to_string(),
        ));
    }

    // Input validation
    if let Some(ref preserve) = params.preserve {
        if preserve.len() > 100 {
            return Err(crate::error::AppError::BadRequest(
                "Preserve name too long (max 100 chars)".to_string(),
            ));
        }
    }

    if let Some(ref after) = params.after {
        if chrono::DateTime::parse_from_rfc3339(after).is_err() {
            return Err(crate::error::AppError::BadRequest(
                "Invalid 'after' date format (must be ISO 8601)".to_string(),
            ));
        }
    }

    let activities = if let Some(preserve_name) = params.preserve {
        // Query by preserve using the join collection
        let results: Vec<ActivityPreserve> = state
            .db
            .get_activities_for_preserve(user.athlete_id, &preserve_name)
            .await?;

        // Map to summaries
        let summaries: Vec<ActivitySummary> = results
            .into_iter()
            .map(|r| ActivitySummary {
                id: r.activity_id,
                name: r.activity_name,
                sport_type: r.sport_type,
                start_date: r.start_date,
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

        (paged_activities, total_count)
    } else {
        // Query Firestore for user's activities
        // Use checked multiplication to prevent u32 overflow (DoS risk)
        let offset = (params.page - 1).checked_mul(limit).ok_or_else(|| {
            crate::error::AppError::BadRequest("Page number causes overflow".to_string())
        })?;

        let results = state
            .db
            .get_activities_for_user(user.athlete_id, params.after.as_deref(), limit, offset)
            .await?;

        let page_count = results.len() as u32;
        // For Firestore queries, getting exact total is expensive.
        // If we got a full page, assume there might be more.
        // We'll return (offset + page_count) + (1 if full page else 0) as a hint,
        // or just return 0 to indicate "unknown" if that's allowed.
        // For now, let's just return the current fetched count + offset as a lower bound
        // or effectively disable "total" based logic for this path until we add aggregation.
        // A common pattern is to return `offset + page_count` if `page_count < per_page`,
        // else `offset + page_count + 1` (at least one more).
        let estimated_total = offset
            .saturating_add(page_count)
            .saturating_add(if page_count == limit { 1 } else { 0 });

        let summaries = results
            .into_iter()
            .map(|a| ActivitySummary {
                id: a.strava_activity_id,
                name: a.name,
                sport_type: a.sport_type,
                start_date: a.start_date,
                preserves: a.preserves_visited,
            })
            .collect();

        (summaries, estimated_total)
    };

    Ok(Json(ActivitiesResponse {
        total: activities.1,
        activities: activities.0,
        page: params.page,
        per_page: limit,
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
        pending_activities: stats.pending_activities,
        available_years,
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_pagination_overflow() {
        let page = u32::MAX;
        let limit = 100;

        // Simulate in-memory pagination calculation
        // On 64-bit systems, this calculation won't overflow usize (u64), which is fine.
        // It produces a huge offset which is safely handled by `if start < len` checks.
        // On 32-bit systems, it would overflow and return None.
        let start_res = (page as usize - 1).checked_mul(limit as usize);
        if std::mem::size_of::<usize>() == 4 {
            assert!(start_res.is_none(), "Should overflow on 32-bit systems");
        } else {
            assert!(start_res.is_some(), "Should fit in usize on 64-bit systems");
        }

        // Simulate DB pagination calculation (u32 math)
        // This MUST always overflow u32::MAX * 100
        let offset_res = (page - 1).checked_mul(limit);
        assert!(
            offset_res.is_none(),
            "Should always overflow u32 (DB query)"
        );
    }

    #[test]
    fn test_pagination_underflow() {
        // Test behavior for page=0 if it wraps (release mode behavior simulation)
        // In debug mode, 0-1 panics. In release, it wraps to u32::MAX.
        // We verify that if it wraps, it triggers the overflow check.
        let wrapped_page = 0u32.wrapping_sub(1); // u32::MAX
        let limit = 100;

        let offset_res = wrapped_page.checked_mul(limit);
        assert!(
            offset_res.is_none(),
            "Should catch wrapped underflow as overflow"
        );
    }
}
