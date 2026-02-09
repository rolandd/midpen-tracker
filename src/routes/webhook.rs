// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Webhook routes for Strava events.

use crate::services::tasks::{DeleteActivityPayload, DeleteUserPayload, ProcessActivityPayload};
use crate::AppState;
use axum::{
    extract::{DefaultBodyLimit, Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use subtle::ConstantTimeEq;

/// Webhook routes.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/webhook/{uuid}", get(verify).post(handle_event))
        // Strava payloads are small (<1KB), so 16KB is a safe conservative limit
        .layer(DefaultBodyLimit::max(16384))
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
    // Validate Path UUID (constant-time comparison to prevent timing attacks)
    if !bool::from(
        uuid.as_bytes()
            .ct_eq(state.config.webhook_path_uuid.as_bytes()),
    ) {
        tracing::warn!(
            received_uuid = %uuid,
            "Security Alert: Webhook path UUID mismatch (verify)"
        );
        return (StatusCode::NOT_FOUND, Json(VerifyResponse::default()));
    }

    if params.mode == "subscribe"
        && bool::from(
            params
                .verify_token
                .as_bytes()
                .ct_eq(state.config.webhook_verify_token.as_bytes()),
        )
    {
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
    body: Bytes,
) -> StatusCode {
    // Validate Path UUID (constant-time comparison to prevent timing attacks)
    // CRITICAL: Check this BEFORE parsing the body to prevent DoS via large payloads
    if !bool::from(
        uuid.as_bytes()
            .ct_eq(state.config.webhook_path_uuid.as_bytes()),
    ) {
        tracing::warn!(
            received_uuid = %uuid,
            "Security Alert: Webhook path UUID mismatch (handle_event)"
        );
        return StatusCode::NOT_FOUND;
    }

    let event: WebhookEvent = match serde_json::from_slice(&body) {
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
            // Queue activity deletion via Cloud Tasks (Verify-before-Act in handler)
            let payload = DeleteActivityPayload {
                activity_id: event.object_id,
                athlete_id: event.owner_id,
                source: "webhook".to_string(),
            };

            if let Err(e) = state
                .tasks_service
                .queue_delete_activity(&state.config.api_url, payload)
                .await
            {
                tracing::error!(error = %e, activity_id = event.object_id, "Failed to queue activity deletion");
            } else {
                tracing::info!(activity_id = event.object_id, "Activity deletion queued");
            }
        }
        ("athlete", "update") if is_deauthorization(&event) => {
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
