//! API routes for authenticated users.

use crate::error::Result;
use crate::middleware::auth::AuthUser;
use crate::models::preserve::PreserveSummary;
use crate::models::ActivityPreserve;
use crate::AppState;
use axum::{
    extract::{Query, State},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// API routes (require authentication via JWT).
/// The auth middleware is applied in routes/mod.rs for these routes.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/me", get(get_me))
        .route("/api/activities", get(get_activities))
        .route("/api/stats/preserves", get(get_preserve_stats))
        .route("/auth/logout", post(logout))
}

// ─── User Profile ────────────────────────────────────────────

/// Current user response.
#[derive(Serialize)]
struct MeResponse {
    athlete_id: u64,
    firstname: String,
    lastname: String,
    profile_picture: Option<String>,
}

/// Get current user profile.
async fn get_me(
    State(_state): State<Arc<AppState>>,
    Extension(user): Extension<AuthUser>,
) -> Result<Json<MeResponse>> {
    // TODO: Look up user profile from Firestore
    Ok(Json(MeResponse {
        athlete_id: user.athlete_id,
        firstname: "TODO".to_string(),
        lastname: "User".to_string(),
        profile_picture: None,
    }))
}

/// Logout (clear session - client should discard token).
async fn logout(Extension(user): Extension<AuthUser>) -> Result<Json<serde_json::Value>> {
    tracing::info!(athlete_id = user.athlete_id, "User logged out");
    // JWT is stateless, so we just return success
    // Client is responsible for discarding the token
    Ok(Json(serde_json::json!({ "success": true })))
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
    20
}

#[derive(Serialize)]
struct ActivitiesResponse {
    activities: Vec<ActivitySummary>,
    page: u32,
    per_page: u32,
    total: u32,
}

#[derive(Serialize, Clone, Debug)]
struct ActivitySummary {
    id: u64,
    name: String,
    sport_type: String,
    start_date: String,
    preserves: Vec<String>,
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

        // Pagination (simple in-memory for now since these lists are small per preserve)
        let start = ((params.page - 1) * params.per_page) as usize;
        if start < summaries.len() {
            let end = (start + params.per_page as usize).min(summaries.len());
            summaries[start..end].to_vec()
        } else {
            vec![]
        }
    } else {
        // Fallback for "all activities" (not implemented yet)
        // TODO: Query Firestore for user's activities
        vec![]
    };

    Ok(Json(ActivitiesResponse {
        total: activities.len() as u32, // Approximation for now
        activities,
        page: params.page,
        per_page: params.per_page,
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
struct PreserveStatsResponse {
    /// All-time preserve visit counts
    preserves: Vec<PreserveSummary>,
    /// Preserve visits broken down by year: { "2025": { "Rancho": 5 } }
    preserves_by_year: std::collections::HashMap<String, std::collections::HashMap<String, u32>>,
    total_preserves_visited: u32,
    total_preserves: u32,
    /// Number of activities still being processed in backfill
    pending_activities: u32,
    /// Available years for filtering (sorted descending)
    available_years: Vec<String>,
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
