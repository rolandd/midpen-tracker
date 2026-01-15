//! API routes for authenticated users.

use crate::error::Result;
use crate::middleware::auth::AuthUser;
use crate::models::preserve::PreserveSummary;
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

#[derive(Serialize)]
struct ActivitySummary {
    id: u64,
    name: String,
    sport_type: String,
    start_date: String,
    preserves: Vec<String>,
}

/// Get user's activities with optional filtering.
async fn get_activities(
    State(_state): State<Arc<AppState>>,
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

    // TODO: Query Firestore for user's activities
    Ok(Json(ActivitiesResponse {
        activities: vec![],
        page: params.page,
        per_page: params.per_page,
        total: 0,
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
    preserves: Vec<PreserveSummary>,
    total_preserves_visited: u32,
    total_preserves: u32,
    /// Number of activities still being processed in backfill
    pending_activities: u32,
}

/// Get preserve visit stats for current user.
///
/// Uses pre-computed aggregates from `user_stats` collection (1 read).
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

    // Build preserve summaries from aggregate data
    let mut preserves: Vec<PreserveSummary> = if params.show_unvisited {
        // Include all preserves, with counts from stats
        all_preserves
            .iter()
            .map(|p| {
                let count = stats.preserves.get(&p.name).copied().unwrap_or(0);
                PreserveSummary {
                    name: p.name.clone(),
                    count,
                    activities: vec![], // Would require separate query for details
                }
            })
            .collect()
    } else {
        // Only visited preserves
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

    // Sort by count descending, then by name
    preserves.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));

    let visited_count = preserves.iter().filter(|p| p.count > 0).count() as u32;

    Ok(Json(PreserveStatsResponse {
        preserves,
        total_preserves_visited: visited_count,
        total_preserves,
        pending_activities: stats.pending_activities,
    }))
}
