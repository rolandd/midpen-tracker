//! Webhook routes for Strava events.

use crate::services::tasks::ProcessActivityPayload;
use crate::AppState;
use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Webhook routes.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/webhook", get(verify).post(handle_event))
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
#[derive(Serialize)]
struct VerifyResponse {
    #[serde(rename = "hub.challenge")]
    challenge: String,
}

/// Verify webhook subscription (GET).
async fn verify(
    State(state): State<Arc<AppState>>,
    Query(params): Query<VerifyParams>,
) -> impl IntoResponse {
    if params.mode == "subscribe" && params.verify_token == state.config.webhook_verify_token {
        tracing::info!("Webhook subscription verified");
        Json(VerifyResponse {
            challenge: params.challenge,
        })
    } else {
        tracing::warn!(
            mode = %params.mode,
            "Webhook verification failed: invalid token"
        );
        Json(VerifyResponse {
            challenge: String::new(),
        })
    }
}

/// Strava webhook event payload.
#[derive(Deserialize, Debug)]
struct WebhookEvent {
    object_type: String, // "activity" or "athlete"
    object_id: u64,
    aspect_type: String, // "create", "update", "delete"
    owner_id: u64,
    #[allow(dead_code)]
    updates: Option<serde_json::Value>,
}

/// Handle incoming webhook events (POST).
async fn handle_event(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(event): Json<WebhookEvent>,
) -> StatusCode {
    tracing::info!(
        object_type = %event.object_type,
        object_id = event.object_id,
        aspect_type = %event.aspect_type,
        owner_id = event.owner_id,
        "Webhook event received"
    );

    match (event.object_type.as_str(), event.aspect_type.as_str()) {
        ("activity", "create") => {
            // Queue activity for processing via Cloud Tasks
            let payload = ProcessActivityPayload {
                activity_id: event.object_id,
                athlete_id: event.owner_id,
                source: "webhook".to_string(),
            };

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
                .queue_activity(&service_url, payload)
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
        ("athlete", "deauthorize") => {
            // Delete user tokens from Firestore
            if let Err(e) = state.db.delete_tokens(event.owner_id).await {
                tracing::error!(error = %e, athlete_id = event.owner_id, "Failed to delete tokens");
            } else {
                tracing::info!(
                    athlete_id = event.owner_id,
                    "User deauthorized, tokens removed"
                );
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
