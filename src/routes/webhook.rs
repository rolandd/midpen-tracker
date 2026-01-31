// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Webhook routes for Strava events.

use crate::error::AppError;
use crate::services::strava::StravaService;
use crate::services::tasks::{DeleteUserPayload, ProcessActivityPayload};
use crate::AppState;
use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Webhook routes.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/webhook/{uuid}", get(verify).post(handle_event))
}

/// Create a StravaService from app state.
/// Helper to avoid duplicating the KMS initialization logic.
async fn create_strava_service(state: &AppState) -> Result<StravaService, AppError> {
    StravaService::new(
        state.config.strava_client_id.clone(),
        state.config.strava_client_secret.clone(),
        state.db.clone(),
        state.config.gcp_project_id.clone(),
        state.config.gcp_region.clone(),
        "token-encryption".to_string(),
    )
    .await
}

/// Strava webhook verification query params.
#[derive(Deserialize)]
struct VerifyParams {
    #[serde(rename = "hub.mode")]
    mode: String,
    #[serde(rename = "hub.challenge")]
    challenge: String,
    #[serde(rename = "hub.verify_token")]
    verify_token: String,
}

/// Verification response.
#[derive(Serialize, Default)]
struct VerifyResponse {
    #[serde(rename = "hub.challenge")]
    challenge: String,
}

/// Verify webhook subscription (GET).
async fn verify(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
    Query(params): Query<VerifyParams>,
) -> impl IntoResponse {
    // Validate Path UUID
    if uuid != state.config.webhook_path_uuid {
        tracing::warn!(
            received_uuid = %uuid,
            "Security Alert: Webhook path UUID mismatch (verify)"
        );
        return (StatusCode::NOT_FOUND, Json(VerifyResponse::default()));
    }

    if params.mode == "subscribe" && params.verify_token == state.config.webhook_verify_token {
        tracing::info!("Webhook subscription verified");
        (
            StatusCode::OK,
            Json(VerifyResponse {
                challenge: params.challenge,
            }),
        )
    } else {
        tracing::warn!(
            mode = %params.mode,
            "Webhook verification failed: invalid token"
        );
        (StatusCode::FORBIDDEN, Json(VerifyResponse::default()))
    }
}

/// Strava webhook event payload.
#[derive(Deserialize, Debug)]
struct WebhookEvent {
    object_type: String, // "activity" or "athlete"
    object_id: u64,
    aspect_type: String, // "create", "update", "delete"
    owner_id: u64,
    subscription_id: u64,
    /// For athlete events, contains {"authorized": "false"} on deauthorization
    #[serde(default)]
    updates: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// Check if a webhook event represents an athlete deauthorization.
/// Strava sends: object_type="athlete", aspect_type="update", updates={"authorized": "false"}
fn is_deauthorization(event: &WebhookEvent) -> bool {
    event
        .updates
        .as_ref()
        .and_then(|u| u.get("authorized"))
        .is_some_and(|v| v == false || v == "false")
}

/// Handle incoming webhook events (POST).
async fn handle_event(
    State(state): State<Arc<AppState>>,
    Path(uuid): Path<String>,
    _headers: axum::http::HeaderMap,
    Json(payload): Json<serde_json::Value>,
) -> StatusCode {
    tracing::info!(
        payload = %payload,
        "Webhook event received (raw)"
    );

    // Validate Path UUID
    if uuid != state.config.webhook_path_uuid {
        tracing::warn!(
            received_uuid = %uuid,
            "Security Alert: Webhook path UUID mismatch (handle_event)"
        );
        return StatusCode::NOT_FOUND;
    }

    let event: WebhookEvent = match serde_json::from_value(payload) {
        Ok(e) => e,
        Err(e) => {
            tracing::error!(error = %e, "Failed to parse webhook event");
            return StatusCode::OK; // Still return 200 to Strava to avoid retries
        }
    };

    // Validate Subscription ID
    if event.subscription_id != state.config.strava_subscription_id {
        tracing::warn!(
            received_id = event.subscription_id,
            expected_id = state.config.strava_subscription_id,
            "Security Alert: Webhook subscription ID mismatch"
        );
        return StatusCode::FORBIDDEN;
    }

    tracing::info!(
        object_type = %event.object_type,
        object_id = event.object_id,
        aspect_type = %event.aspect_type,
        owner_id = event.owner_id,
        "Webhook event parsed successfully"
    );

    match (event.object_type.as_str(), event.aspect_type.as_str()) {
        ("activity", "create") => {
            // Queue activity for processing via Cloud Tasks
            let payload = ProcessActivityPayload {
                activity_id: event.object_id,
                athlete_id: event.owner_id,
                source: "webhook".to_string(),
            };

            if let Err(e) = state
                .tasks_service
                .queue_activity(&state.config.api_url, payload)
                .await
            {
                tracing::error!(error = %e, "Failed to queue activity");
            }
        }
        ("activity", "update") => {
            // Could re-process if needed, but for now just log
            tracing::debug!(activity_id = event.object_id, "Activity updated");
        }
        ("activity", "delete") => {
            // Verify activity against Strava API to prevent unauthorized deletion
            let should_process = match create_strava_service(&state).await {
                Ok(strava) => {
                    // Try to fetch the activity.
                    match strava.get_activity(event.owner_id, event.object_id).await {
                        Ok(_) => {
                            // Activity found -> Deletion webhook is FAKE
                            tracing::warn!(
                                activity_id = event.object_id,
                                athlete_id = event.owner_id,
                                "Security Alert: Received FAKE activity deletion webhook (activity still exists)"
                            );
                            false
                        }
                        Err(AppError::NotFound(_)) => {
                            // Activity not found in our DB -> Deletion is "real" (or at least irrelevant)
                            true
                        }
                        Err(AppError::StravaApi(ref s)) if s.contains("404") => {
                            // Activity not found on Strava -> Deletion is REAL
                            true
                        }
                        Err(AppError::StravaApi(ref s))
                            if s.contains("Token expired") || s.contains("invalid") =>
                        {
                            // User revoked access -> Treat as real deletion (or at least harmless)
                            true
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "Failed to verify activity deletion (assuming real)"
                            );
                            true
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create StravaService for verification");
                    true // Proceed cautiously
                }
            };

            if should_process {
                // Delete activity and stats from Firestore
                if let Err(e) = state
                    .db
                    .delete_activity(event.object_id, event.owner_id)
                    .await
                {
                    tracing::error!(error = %e, activity_id = event.object_id, "Failed to delete activity");
                } else {
                    tracing::info!(activity_id = event.object_id, "Activity deleted");
                }
            }
        }
        ("athlete", "update") if is_deauthorization(&event) => {
            // Verify user token against Strava API to prevent unauthorized deletion
            let should_process = match create_strava_service(&state).await {
                Ok(strava) => {
                    // Try to get a valid token (refreshes if needed)
                    match strava.get_valid_access_token(event.owner_id).await {
                        Ok(_) => {
                            // Token is valid -> User is authorized -> Deauth webhook is FAKE
                            tracing::warn!(
                                athlete_id = event.owner_id,
                                "Security Alert: Received FAKE deauthorization webhook (token still valid)"
                            );
                            false
                        }
                        Err(AppError::StravaApi(ref s))
                            if s.contains("Token expired") || s.contains("invalid") =>
                        {
                            // Token revoked -> Deauth is REAL
                            true
                        }
                        Err(AppError::NotFound(_)) => {
                            // No tokens in DB -> User already gone or never authed -> Safe to proceed
                            true
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                "Failed to verify deauthorization (assuming real)"
                            );
                            true
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to create StravaService for verification");
                    true
                }
            };

            if should_process {
                // Queue user deletion via Cloud Tasks (respond immediately to Strava)
                let payload = DeleteUserPayload {
                    athlete_id: event.owner_id,
                    source: "webhook".to_string(),
                };

                if let Err(e) = state
                    .tasks_service
                    .queue_delete_user(&state.config.api_url, payload)
                    .await
                {
                    tracing::error!(error = %e, athlete_id = event.owner_id, "Failed to queue user deletion");
                } else {
                    tracing::info!(athlete_id = event.owner_id, "User deletion queued");
                }
            }
        }
        _ => {
            tracing::debug!(
                object_type = %event.object_type,
                aspect_type = %event.aspect_type,
                "Ignoring unhandled event type"
            );
        }
    }

    // Always return 200 OK quickly (Strava requirement)
    StatusCode::OK
}
