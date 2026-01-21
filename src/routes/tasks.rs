//! Task handler routes for Cloud Tasks callbacks.
//!
//! These endpoints are called by Cloud Tasks, not directly by users.
//! They should be protected by OIDC token verification in production.

use crate::error::AppError;
use crate::models::UserStats;
use crate::services::activity::ActivityProcessor;
use crate::services::kms::KmsService;
use crate::services::strava::StravaService;
use crate::services::tasks::{ContinueBackfillPayload, ProcessActivityPayload};
use crate::AppState;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::post,
    Router,
};
use std::sync::Arc;

/// Task handler routes (called by Cloud Tasks).
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/tasks/process-activity", post(process_activity))
        .route("/tasks/continue-backfill", post(continue_backfill))
}

/// Create a StravaService from app state.
/// Helper to avoid duplicating the KMS initialization logic.
async fn create_strava_service(state: &AppState) -> Result<StravaService, AppError> {
    let kms = KmsService::new(
        &state.config.gcp_project_id,
        "us-west1",
        "midpen-strava",
        "token-encryption",
    )
    .await?;

    Ok(StravaService::new(
        state.config.strava_client_id.clone(),
        state.config.strava_client_secret.clone(),
        state.db.clone(),
        kms,
    ))
}

/// Process a single activity (called by Cloud Tasks).
async fn process_activity(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<ProcessActivityPayload>,
) -> StatusCode {
    // Security Check: Ensure request comes from Cloud Tasks
    // Cloud Run strips this header from external requests, so its presence guarantees internal origin.
    if !headers.contains_key("x-cloudtasks-queuename") {
        tracing::warn!(
            activity_id = payload.activity_id,
            athlete_id = payload.athlete_id,
            "Security Alert: Blocked unauthorized access to process_activity (missing x-cloudtasks-queuename header)"
        );
        return StatusCode::FORBIDDEN;
    }

    tracing::info!(
        activity_id = payload.activity_id,
        athlete_id = payload.athlete_id,
        source = %payload.source,
        "Processing activity from Cloud Task"
    );

    // Create StravaService
    let strava = match create_strava_service(&state).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create StravaService");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    // Create activity processor
    let processor =
        ActivityProcessor::new(strava, state.preserve_service.clone(), state.db.clone());

    // Process the activity
    match processor
        .process_activity(payload.athlete_id, payload.activity_id, &payload.source)
        .await
    {
        Ok(result) => {
            tracing::info!(
                activity_id = payload.activity_id,
                preserves = ?result.preserves_visited,
                "Activity processed successfully"
            );

            // Decrement pending count on success
            if let Err(e) = decrement_pending(&state, payload.athlete_id).await {
                tracing::warn!(error = %e, "Failed to decrement pending count");
            }

            StatusCode::OK
        }
        Err(e) => {
            tracing::error!(
                activity_id = payload.activity_id,
                error = %e,
                "Failed to process activity"
            );
            // Return 500 to trigger Cloud Tasks retry
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

/// Continue backfill by fetching the next page of activities.
/// This spreads Strava API calls over time via Cloud Tasks rate limiting.
async fn continue_backfill(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<ContinueBackfillPayload>,
) -> StatusCode {
    // Security Check: Ensure request comes from Cloud Tasks
    if !headers.contains_key("x-cloudtasks-queuename") {
        tracing::warn!(
            athlete_id = payload.athlete_id,
            "Security Alert: Blocked unauthorized access to continue_backfill (missing x-cloudtasks-queuename header)"
        );
        return StatusCode::FORBIDDEN;
    }

    tracing::info!(
        athlete_id = payload.athlete_id,
        page = payload.next_page,
        "Continuing backfill from Cloud Task"
    );

    // Create StravaService (handles token refresh automatically)
    let strava = match create_strava_service(&state).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create StravaService");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    // Fetch next page of activities (token refresh is handled by StravaService)
    let per_page = 100u32;
    let activities = match strava
        .list_activities(
            payload.athlete_id,
            payload.after_timestamp,
            payload.next_page,
            per_page,
        )
        .await
    {
        Ok(a) => a,
        Err(AppError::NotFound(_)) => {
            // User may have disconnected - don't retry
            tracing::error!(athlete_id = payload.athlete_id, "No tokens for backfill");
            return StatusCode::OK;
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch activities from Strava for backfill");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    if activities.is_empty() {
        tracing::info!(
            athlete_id = payload.athlete_id,
            "Backfill complete - no more activities"
        );
        return StatusCode::OK;
    }

    // Filter out already processed activities
    let stats = match state.db.get_user_stats(payload.athlete_id).await {
        Ok(Some(s)) => s,
        Ok(None) => UserStats::default(),
        Err(e) => {
            tracing::warn!(error = %e, "Failed to fetch stats for duplicate check");
            UserStats::default()
        }
    };

    let new_activity_ids: Vec<u64> = activities
        .iter()
        .map(|a| a.id)
        .filter(|id| !stats.processed_activity_ids.contains(id))
        .collect();

    let total_fetched = activities.len();
    let count = new_activity_ids.len();

    if count > 0 {
        // Update pending count (add newly queued activities)
        if let Err(e) = increment_pending(&state, payload.athlete_id, count as u32).await {
            tracing::warn!(error = %e, "Failed to increment pending count");
        }
    } else {
        tracing::info!(
            athlete_id = payload.athlete_id,
            page = payload.next_page,
            "All fetched activities on this page already processed"
        );
    }

    // Construct service URL for Cloud Tasks
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost:8080");

    let scheme = if host.contains("localhost") || host.contains("127.0.0.1") {
        "http"
    } else {
        "https"
    };
    let service_url = format!("{}://{}", scheme, host);

    if let Err(e) = state
        .tasks_service
        .queue_backfill(&service_url, payload.athlete_id, new_activity_ids)
        .await
    {
        tracing::error!(error = %e, "Failed to queue activities for backfill");

        // Rollback pending count to avoid "phantom" backlog
        let mut stats = match state.db.get_user_stats(payload.athlete_id).await {
            Ok(s) => s.unwrap_or_else(UserStats::default),
            Err(err) => {
                tracing::error!(error = %err, "Failed to fetch stats for rollback");
                UserStats::default()
            }
        };
        if stats.pending_activities >= count as u32 {
            stats.pending_activities -= count as u32;
            stats.updated_at = chrono::Utc::now().to_rfc3339();
            if let Err(db_err) = state.db.set_user_stats(payload.athlete_id, &stats).await {
                tracing::error!(error = %db_err, "Failed to rollback pending count");
            }
        }

        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    // If we got a full page, there might be more - queue next page
    if total_fetched >= per_page as usize {
        let next_payload = ContinueBackfillPayload {
            athlete_id: payload.athlete_id,
            next_page: payload.next_page + 1,
            after_timestamp: payload.after_timestamp,
        };

        if let Err(e) = state
            .tasks_service
            .queue_continue_backfill(&service_url, next_payload)
            .await
        {
            tracing::error!(error = %e, "Failed to queue next backfill page");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    } else {
        // Backfill scan complete (fetched less than full page)
        // Self-Healing: Hard reset pending count to 0 to ensure consistency.
        // This fixes cases where the count gets stuck due to lost tasks or errors.
        tracing::info!(
            athlete_id = payload.athlete_id,
            "Backfill scan completed, resetting pending count"
        );

        let mut stats = match state.db.get_user_stats(payload.athlete_id).await {
            Ok(s) => s.unwrap_or_else(UserStats::default),
            Err(err) => {
                tracing::warn!(error = %err, "Failed to fetch stats for reset");
                UserStats::default()
            }
        };
        stats.pending_activities = 0;
        stats.updated_at = chrono::Utc::now().to_rfc3339();

        if let Err(e) = state.db.set_user_stats(payload.athlete_id, &stats).await {
            tracing::warn!(error = %e, "Failed to reset pending count");
        }
    }

    tracing::info!(
        athlete_id = payload.athlete_id,
        page = payload.next_page,
        count,
        "Queued backfill activities from page"
    );

    StatusCode::OK
}

/// Decrement the pending activities count after successfully processing one.
async fn decrement_pending(
    state: &Arc<AppState>,
    athlete_id: u64,
) -> Result<(), crate::error::AppError> {
    let mut stats = state
        .db
        .get_user_stats(athlete_id)
        .await?
        .unwrap_or_else(UserStats::default);

    if stats.pending_activities > 0 {
        stats.pending_activities -= 1;
        stats.updated_at = chrono::Utc::now().to_rfc3339();
        state.db.set_user_stats(athlete_id, &stats).await?;
    }

    Ok(())
}

/// Increment the pending activities count when queuing new activities.
async fn increment_pending(
    state: &Arc<AppState>,
    athlete_id: u64,
    count: u32,
) -> Result<(), crate::error::AppError> {
    let mut stats = state
        .db
        .get_user_stats(athlete_id)
        .await?
        .unwrap_or_else(UserStats::default);

    stats.pending_activities += count;
    stats.updated_at = chrono::Utc::now().to_rfc3339();
    state.db.set_user_stats(athlete_id, &stats).await?;

    Ok(())
}
