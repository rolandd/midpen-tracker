// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Task handler routes for Cloud Tasks callbacks.
//!
//! These endpoints are called by Cloud Tasks, not directly by users.
//! They are protected by OIDC token verification via middleware.

use crate::error::AppError;
use crate::models::UserStats;
use crate::services::activity::ActivityProcessor;
use crate::services::tasks::{
    BackfillResult, ContinueBackfillPayload, DeleteActivityPayload, DeleteUserPayload,
    ProcessActivityPayload,
};
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
        .route("/tasks/delete-user", post(delete_user))
        .route("/tasks/delete-activity", post(delete_activity))
}

/// Process a single activity (called by Cloud Tasks).
async fn process_activity(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ProcessActivityPayload>,
) -> StatusCode {
    tracing::info!(
        activity_id = payload.activity_id,
        athlete_id = payload.athlete_id,
        source = %payload.source,
        "Processing activity from Cloud Task"
    );

    // Use shared StravaService
    let strava = state.strava_service.clone();

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

            StatusCode::OK
        }
        Err(AppError::NotFound(msg)) if msg.contains("Tokens") || msg.contains("User") => {
            tracing::warn!(
                activity_id = payload.activity_id,
                athlete_id = payload.athlete_id,
                error = %msg,
                "User/Tokens not found during processing - stopping retry (user likely deleted)"
            );
            // Decrement pending count since this task is finished (terminally failed)
            let _ = state.db.update_pending_count(payload.athlete_id, -1).await;
            StatusCode::OK
        }
        Err(e) if e.is_strava_token_error() => {
            tracing::warn!(
                activity_id = payload.activity_id,
                athlete_id = payload.athlete_id,
                error = %e,
                "Token revoked - stopping retry (user likely deauthorized)"
            );
            // Decrement pending count since this task is finished (terminally failed)
            let _ = state.db.update_pending_count(payload.athlete_id, -1).await;
            StatusCode::OK // Stop retrying - will never succeed
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
    Json(payload): Json<ContinueBackfillPayload>,
) -> StatusCode {
    tracing::info!(
        athlete_id = payload.athlete_id,
        page = payload.next_page,
        "Continuing backfill from Cloud Task"
    );

    // Use shared StravaService (handles token refresh automatically)
    let strava = state.strava_service.clone();

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
            tracing::warn!(athlete_id = payload.athlete_id, "No tokens for backfill");
            return StatusCode::OK;
        }
        Err(e) if e.is_strava_token_error() => {
            tracing::warn!(
                athlete_id = payload.athlete_id,
                error = %e,
                "Token revoked during backfill - stopping retry"
            );
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

    let mut queue_error = false;

    if count == 0 {
        tracing::info!(
            athlete_id = payload.athlete_id,
            page = payload.next_page,
            "All fetched activities on this page already processed"
        );
    } else {
        // Queue activities first, then update pending count based on actual success
        let backfill_result = state
            .tasks_service
            .queue_backfill(&state.config.api_url, payload.athlete_id, new_activity_ids)
            .await;

        // Handle results based on what actually happened.
        // Idempotent task queuing ensures we don't create duplicate tasks on retry.
        if let Err(e) = handle_backfill_result(&state, payload.athlete_id, &backfill_result).await {
            tracing::error!(error = %e, "Failed to update pending count for backfill page");
            queue_error = true;
        } else if backfill_result.failed > 0 {
            tracing::error!(
                athlete_id = payload.athlete_id,
                requested = backfill_result.requested,
                queued = backfill_result.queued,
                failed = backfill_result.failed,
                failed_ids = ?&backfill_result.failed_ids.iter().take(20).collect::<Vec<_>>(),
                "Some activities failed to queue for backfill (failed_ids may be truncated)"
            );
            queue_error = true;
        }
    }

    // If we got a full page, there might be more - queue next page.
    // We attempt to queue the next page even if the current page had partial failures,
    // to ensure the overall backfill scan completes even if some individual tasks are lost.
    if total_fetched >= per_page as usize {
        // Fix: Use checked_add to prevent u32 overflow (infinite loop risk)
        let next_page = match payload.next_page.checked_add(1) {
            Some(p) => p,
            None => {
                tracing::warn!(
                    athlete_id = payload.athlete_id,
                    "Backfill page limit reached (u32 overflow) - stopping"
                );
                return StatusCode::OK;
            }
        };

        let next_payload = ContinueBackfillPayload {
            athlete_id: payload.athlete_id,
            next_page,
            after_timestamp: payload.after_timestamp,
        };

        if let Err(e) = state
            .tasks_service
            .queue_continue_backfill(&state.config.api_url, next_payload)
            .await
        {
            tracing::error!(error = %e, "Failed to queue next backfill page");
            // If we can't queue the next page, we MUST retry this task
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    // If queuing failed for some activities on this page, return an error to trigger 
    // a Cloud Tasks retry of the current page. The next page has already been 
    // queued, but retrying this page is safe due to idempotent task names.
    if queue_error {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    tracing::info!(
        athlete_id = payload.athlete_id,
        page = payload.next_page,
        count,
        "Queued backfill activities from page"
    );

    StatusCode::OK
}

/// Increment the pending activities count when queuing new activities.
///
/// Uses a Firestore transaction to prevent lost updates from concurrent
/// backfill or webhook operations.
async fn increment_pending(
    state: &Arc<AppState>,
    athlete_id: u64,
    count: u32,
) -> Result<(), crate::error::AppError> {
    state
        .db
        .update_pending_count(athlete_id, count as i64)
        .await
}

/// Handle the result of a backfill queue operation.
///
/// Updates pending count only for activities that were actually queued,
/// ensuring accuracy even in partial failure scenarios.
pub(crate) async fn handle_backfill_result(
    state: &Arc<AppState>,
    athlete_id: u64,
    result: &BackfillResult,
) -> Result<(), crate::error::AppError> {
    // Only increment pending count by what was actually newly queued.
    // Idempotent (AlreadyExists) successes should not be counted again.
    if result.newly_queued > 0 {
        increment_pending(state, athlete_id, result.newly_queued).await?;
    }

    // Log details about failures for debugging
    if result.failed > 0 {
        tracing::warn!(
            athlete_id,
            failed_count = result.failed,
            failed_ids = ?&result.failed_ids.iter().take(20).collect::<Vec<_>>(),
            "Some activities failed to queue for backfill (failed_ids may be truncated)"
        );
    }

    Ok(())
}

/// Delete a user and all their data (GDPR compliance).
/// Called by Cloud Tasks from webhook deauthorization or user-initiated deletion.
///
/// Flow:
/// 1. Get tokens from DB → hold in memory
/// 2. DELETE tokens from DB immediately (blocks concurrent activity processing)
/// 3. Delete all user data (activities, preserves, stats, user)
/// 4. Call Strava deauthorize using in-memory tokens
async fn delete_user(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeleteUserPayload>,
) -> StatusCode {
    tracing::info!(
        athlete_id = payload.athlete_id,
        source = %payload.source,
        "Processing user deletion from Cloud Task"
    );

    // Use shared StravaService
    let strava = state.strava_service.clone();

    // Verify-before-Act: If source is webhook (deauthorization), ensure the token is actually invalid.
    if payload.source == "webhook" {
        // We must verify against the LIVE Strava API, not just our cache.
        // verify_token_active() forces a check if we have a token.
        match strava.verify_token_active(payload.athlete_id).await {
            Ok(true) => {
                // Token worked against API -> User is authorized -> Deauth webhook is FAKE
                tracing::warn!(
                    athlete_id = payload.athlete_id,
                    "Security Alert: Received FAKE deauthorization webhook task (token still valid) - Aborting deletion"
                );
                // Return 200 to stop retry (we successfully handled the "fake" event by ignoring it)
                return StatusCode::OK;
            }
            Ok(false) => {
                // Token rejected by API -> Deauth is REAL -> Proceed
                tracing::info!(
                    athlete_id = payload.athlete_id,
                    "Verified deauthorization via Strava API (token invalid) - Proceeding with deletion"
                );
            }
            Err(AppError::NotFound(_)) => {
                // No tokens in DB -> User already gone or never authed -> Safe to proceed
                tracing::info!(
                    athlete_id = payload.athlete_id,
                    "No tokens found for verification - Proceeding with cleanup"
                );
            }
            Err(e) => {
                tracing::error!(
                    error = %e,
                    athlete_id = payload.athlete_id,
                    "Failed to verify deauthorization status - Retrying later"
                );
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        }
    }

    // 1. Revoke local tokens immediately (get valid token for later deauth)
    // This blocks concurrent activity processing.
    let access_token_opt = match strava.revoke_local_tokens(payload.athlete_id).await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "Failed to revoke tokens");
            // If DB error, we probably can't proceed.
            // If it's just "not found", revoke_local_tokens returns Ok(None).
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    // Safety Check: Did the user re-login immediately?
    // If tokens exist now, it means handle_oauth_callback ran AFTER revoke_local_tokens.
    // We should abort deletion to preserve the new account.
    if let Ok(Some(_)) = state.db.get_tokens(payload.athlete_id).await {
        tracing::warn!(
            athlete_id = payload.athlete_id,
            "User re-logged in during deletion process - ABORTING deletion to preserve new account"
        );
        return StatusCode::OK;
    }

    // 2. Delete all user data
    let deletion_result = state.db.delete_user_data(payload.athlete_id).await;
    match &deletion_result {
        Ok(count) => {
            tracing::info!(
                athlete_id = payload.athlete_id,
                deleted_count = count,
                "User data deleted successfully"
            );
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete user data");
            // Continue to Strava deauth anyway, but will return error later
        }
    }

    // Safety Check Again: Did user login during data deletion?
    if let Ok(Some(_)) = state.db.get_tokens(payload.athlete_id).await {
        tracing::warn!(
            athlete_id = payload.athlete_id,
            "User re-logged in during data deletion - ABORTING deauth"
        );
        return StatusCode::OK;
    }

    // 3. Deauthorize with Strava using the valid token
    if let Some(token) = access_token_opt {
        // If triggered by webhook, the user already revoked access on Strava.
        // Calling deauthorize again might confuse Strava or fail, so we skip it.
        if payload.source == "webhook" {
            tracing::info!(
                athlete_id = payload.athlete_id,
                "Skipping Strava deauthorization (triggered by webhook revocation)"
            );
        } else if let Err(e) = strava.deauthorize_with_token(&token).await {
            tracing::warn!(
                error = %e,
                athlete_id = payload.athlete_id,
                "Failed to deauthorize with Strava (non-fatal)"
            );
        } else {
            tracing::info!(
                athlete_id = payload.athlete_id,
                "Strava deauthorization successful"
            );
        }
    } else {
        tracing::info!(
            athlete_id = payload.athlete_id,
            "No tokens found to deauthorize (already deleted or failed to decrypt)"
        );
    }

    if deletion_result.is_err() {
        // Return 500 to trigger retry if data deletion failed
        // Note: revoke_local_tokens succeeded, so tokens are gone. Retry will hit "No tokens found" branch
        // and focus on deleting data.
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    tracing::info!(
        athlete_id = payload.athlete_id,
        source = %payload.source,
        "User deletion complete"
    );

    StatusCode::OK
}

/// Delete an activity.
/// Called by Cloud Tasks from webhook activity deletion.
async fn delete_activity(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeleteActivityPayload>,
) -> StatusCode {
    tracing::info!(
        activity_id = payload.activity_id,
        athlete_id = payload.athlete_id,
        "Processing activity deletion from Cloud Task"
    );

    // Use shared StravaService
    let strava = state.strava_service.clone();

    // Verify-before-Act: Check if activity still exists on Strava
    match strava
        .get_activity(payload.athlete_id, payload.activity_id)
        .await
    {
        Ok(_) => {
            // Activity found -> Deletion webhook is FAKE
            tracing::warn!(
                activity_id = payload.activity_id,
                athlete_id = payload.athlete_id,
                "Security Alert: Received FAKE activity deletion webhook task (activity still exists) - Aborting deletion"
            );
            return StatusCode::OK;
        }
        Err(AppError::NotFound(_)) => {
            // Activity not found in our DB -> Deletion is "real" (or at least irrelevant)
            tracing::info!(
                activity_id = payload.activity_id,
                "Activity not found locally - proceeding with cleanup"
            );
        }
        Err(AppError::StravaApi(ref s)) if s.contains("404") => {
            // Activity not found on Strava -> Deletion is REAL -> Proceed
            tracing::info!(
                activity_id = payload.activity_id,
                "Verified activity deletion via Strava API (404) - Proceeding"
            );
        }
        Err(e) if e.is_strava_token_error() => {
            // User revoked access -> Treat as real deletion (or at least harmless)
            tracing::info!(
                activity_id = payload.activity_id,
                "User token invalid during deletion verification - Proceeding"
            );
        }
        Err(e) => {
            tracing::error!(
                error = %e,
                activity_id = payload.activity_id,
                "Failed to verify activity deletion status - Retrying later"
            );
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    }

    // Delete activity and stats from Firestore
    if let Err(e) = state
        .db
        .delete_activity(payload.activity_id, payload.athlete_id)
        .await
    {
        tracing::error!(error = %e, activity_id = payload.activity_id, "Failed to delete activity");
        return StatusCode::INTERNAL_SERVER_ERROR;
    } else {
        tracing::info!(activity_id = payload.activity_id, "Activity deleted");
    }

    StatusCode::OK
}
