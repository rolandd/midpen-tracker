//! Strava OAuth authentication routes.

use axum::{
    extract::{Query, State},
    response::Redirect,
    routing::get,
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{AppError, Result};
use crate::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth/strava", get(auth_start))
        .route("/auth/strava/callback", get(auth_callback))
        .route("/auth/logout", get(logout))
}

/// Create a JWT token for the authenticated user.
fn create_jwt(
    athlete_id: u64,
    signing_key: &[u8],
) -> std::result::Result<String, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use serde::Serialize;

    #[derive(Serialize)]
    struct Claims {
        sub: String, // athlete_id as string
        exp: usize,  // expiration timestamp
        iat: usize,  // issued at timestamp
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = Claims {
        sub: athlete_id.to_string(),
        exp: now + 86400 * 30, // 30 days
        iat: now,
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(signing_key),
    )
}

/// Query parameters for starting OAuth flow.
#[derive(Deserialize)]
pub struct AuthStartParams {
    /// Frontend URL to redirect back to after OAuth completes.
    /// If not provided, uses FRONTEND_URL env var.
    #[serde(default)]
    redirect_uri: Option<String>,
}

/// Start OAuth flow - redirect to Strava authorization.
async fn auth_start(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuthStartParams>,
    headers: axum::http::HeaderMap,
) -> Redirect {
    // Get the frontend URL from query param or fall back to config
    let frontend_url = params
        .redirect_uri
        .unwrap_or_else(|| state.config.frontend_url.clone());

    // Encode frontend URL + timestamp in state (URL-safe base64)
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let state_data = format!("{}|{:x}", frontend_url, timestamp);
    let oauth_state = URL_SAFE_NO_PAD.encode(state_data.as_bytes());

    // Get the host from the request headers for callback URL
    let host = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            std::env::var("API_HOST").unwrap_or_else(|_| "localhost:8080".to_string())
        });

    let scheme = if host.contains("localhost") || host.contains("127.0.0.1") {
        "http"
    } else {
        "https"
    };

    let callback_url = format!("{}://{}/auth/strava/callback", scheme, host);

    let auth_url = format!(
        "https://www.strava.com/oauth/authorize?\
         client_id={}&\
         redirect_uri={}&\
         response_type=code&\
         scope=activity:read_all,activity:write&\
         state={}",
        state.config.strava_client_id,
        urlencoding::encode(&callback_url),
        oauth_state
    );

    tracing::info!(
        client_id = %state.config.strava_client_id,
        frontend_url = %frontend_url,
        "Starting OAuth flow, redirecting to Strava"
    );

    Redirect::temporary(&auth_url)
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: String,
    state: String,
    #[serde(default)]
    error: Option<String>,
}

/// Strava's token exchange response.
#[derive(Deserialize, Debug)]
struct StravaTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    athlete: StravaAthlete,
}

#[derive(Deserialize, Debug)]
struct StravaAthlete {
    id: u64,
    firstname: String,
    lastname: String,
    profile: Option<String>,
}

/// OAuth callback - exchange code for tokens, create session.
async fn auth_callback(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<CallbackParams>,
) -> Result<Redirect> {
    // Construct service URL for Cloud Tasks (assumes HTTPS in production)
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
    // Decode frontend URL from state parameter
    let frontend_url =
        decode_state_frontend(&params.state).unwrap_or_else(|| state.config.frontend_url.clone());

    // Check for OAuth errors
    if let Some(error) = params.error {
        tracing::warn!(error = %error, "OAuth error from Strava");
        let redirect = format!("{}?error={}", frontend_url, error);
        return Ok(Redirect::temporary(&redirect));
    }

    tracing::info!("Exchanging authorization code for tokens");

    // Exchange authorization code for access token
    let token_response = exchange_code_for_tokens(
        &params.code,
        &state.config.strava_client_id,
        &state.config.strava_client_secret,
    )
    .await?;

    let athlete_id = token_response.athlete.id;

    tracing::info!(
        athlete_id = athlete_id,
        firstname = %token_response.athlete.firstname,
        "OAuth successful, storing user and tokens"
    );

    // Get current timestamp
    let now = chrono::Utc::now().to_rfc3339();

    // Store user profile in Firestore
    let user = crate::models::User {
        strava_athlete_id: athlete_id,
        email: None,
        firstname: token_response.athlete.firstname.clone(),
        lastname: token_response.athlete.lastname.clone(),
        profile_picture: token_response.athlete.profile.clone(),
        created_at: now.clone(),
        last_active: now.clone(),
    };

    if let Err(e) = state.db.upsert_user(&user).await {
        tracing::warn!(error = %e, "Failed to store user profile, continuing anyway");
    }

    // Create KMS service
    let kms = crate::services::kms::KmsService::new(
        &state.config.gcp_project_id,
        "us-west1",
        "midpen-strava",
        "token-encryption",
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Failed to initialize KMS service");
        e
    })?;

    // Store encrypted tokens in Firestore
    let expires_at = chrono::DateTime::<chrono::Utc>::from_timestamp(token_response.expires_at, 0)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| now.clone());

    // Encrypt tokens
    let (enc_access, enc_refresh) = crate::services::kms::encrypt_tokens(
        &kms,
        &token_response.access_token,
        &token_response.refresh_token,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Token encryption failed");
        e
    })?;

    let tokens = crate::models::UserTokens {
        access_token_encrypted: enc_access,
        refresh_token_encrypted: enc_refresh,
        expires_at,
        scopes: vec![
            "activity:read_all".to_string(),
            "activity:write".to_string(),
        ],
    };

    if let Err(e) = state.db.set_tokens(athlete_id, &tokens).await {
        tracing::warn!(error = %e, "Failed to store tokens, continuing anyway");
    }

    // Queue backfill for activities since 2025-01-01
    let backfill_result = trigger_backfill(
        &state,
        athlete_id,
        &token_response.access_token,
        &service_url,
    )
    .await;

    if let Err(e) = backfill_result {
        tracing::warn!(error = %e, "Failed to trigger backfill, continuing anyway");
    }

    // Create JWT session token
    let jwt = create_jwt(athlete_id, &state.config.jwt_signing_key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT creation failed: {}", e)))?;

    // Redirect to frontend with token
    let redirect_url = format!("{}/callback?token={}", frontend_url, jwt);

    Ok(Redirect::temporary(&redirect_url))
}

/// Trigger backfill for activities since 2025-01-01.
/// Only fetches first page at login, then queues a continue-backfill task
/// for subsequent pages to spread Strava API calls over time.
async fn trigger_backfill(
    state: &Arc<AppState>,
    athlete_id: u64,
    access_token: &str,
    service_url: &str,
) -> Result<()> {
    use crate::models::UserStats;
    use crate::services::strava::StravaClient;
    use crate::services::tasks::ContinueBackfillPayload;

    // Backfill activities since 2025-01-01
    let after_timestamp = chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    let strava = StravaClient::new(
        state.config.strava_client_id.clone(),
        state.config.strava_client_secret.clone(),
    );

    // Fetch ONLY the first page at login (100 activities max)
    let per_page = 100u32;
    let activities = strava
        .list_activities(access_token, after_timestamp, 1, per_page)
        .await?;

    if activities.is_empty() {
        tracing::info!(athlete_id, "No activities to backfill since 2025-01-01");
        return Ok(());
    }

    // Filter out activities that have already been processed
    // This makes the backfill process idempotent (safe to re-run on login)
    let stats = state
        .db
        .get_user_stats(athlete_id)
        .await?
        .unwrap_or_else(UserStats::default);

    let new_activity_ids: Vec<u64> = activities
        .iter()
        .map(|a| a.id)
        .filter(|id| !stats.processed_activity_ids.contains(id))
        .collect();

    let total_fetched = activities.len() as u32;
    let new_count = new_activity_ids.len() as u32;

    if new_count == 0 {
        tracing::info!(
            athlete_id,
            "All {} fetched activities already processed",
            total_fetched
        );
    } else {
        tracing::info!(
            athlete_id,
            total_fetched,
            new_count,
            "Queueing new activities for backfill"
        );

        // Update UserStats pending count (increment, don't overwrite)
        let mut stats_to_update = stats.clone();
        stats_to_update.pending_activities += new_count;
        stats_to_update.updated_at = chrono::Utc::now().to_rfc3339();

        if let Err(e) = state.db.set_user_stats(athlete_id, &stats_to_update).await {
            tracing::warn!(error = %e, "Failed to update pending activities count");
        }

        // Queue only the new activities
        // Queue only the new activities
        if let Err(e) = state
            .tasks_service
            .queue_backfill(service_url, athlete_id, new_activity_ids)
            .await
        {
            // Rollback pending count
            let mut rollback_stats = state
                .db
                .get_user_stats(athlete_id)
                .await?
                .unwrap_or_else(UserStats::default);
            if rollback_stats.pending_activities >= new_count {
                rollback_stats.pending_activities -= new_count;
                rollback_stats.updated_at = chrono::Utc::now().to_rfc3339();
                if let Err(db_err) = state.db.set_user_stats(athlete_id, &rollback_stats).await {
                    tracing::error!(error = %db_err, "Failed to rollback pending count in auth handler");
                }
            }
            return Err(e);
        }
    }

    // If we got a full page, there might be more - queue continue-backfill task
    // This spreads subsequent Strava API calls via Cloud Tasks rate limiting
    if total_fetched >= per_page {
        let continue_payload = ContinueBackfillPayload {
            athlete_id,
            next_page: 2,
            after_timestamp,
        };

        if let Err(e) = state
            .tasks_service
            .queue_continue_backfill(service_url, continue_payload)
            .await
        {
            tracing::warn!(error = %e, "Failed to queue continue-backfill task");
        }
    }

    Ok(())
}

/// Decode the frontend URL from the OAuth state parameter.
fn decode_state_frontend(state: &str) -> Option<String> {
    let bytes = URL_SAFE_NO_PAD.decode(state).ok()?;
    let state_str = String::from_utf8(bytes).ok()?;
    // Format is "frontend_url|timestamp_hex"
    let parts: Vec<&str> = state_str.splitn(2, '|').collect();
    parts.first().map(|s| s.to_string())
}

/// Exchange authorization code for access and refresh tokens.
async fn exchange_code_for_tokens(
    code: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<StravaTokenResponse> {
    let client = reqwest::Client::new();

    let response = client
        .post("https://www.strava.com/oauth/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::StravaApi(format!("Strava token exchange failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        tracing::error!(status = %status, body = %body, "Strava token exchange failed");
        return Err(AppError::StravaApi(format!(
            "Strava token exchange failed with status {}",
            status
        )));
    }

    response
        .json()
        .await
        .map_err(|e| AppError::StravaApi(format!("Failed to parse Strava response: {}", e)))
}

/// Logout - just a placeholder that clears client-side token.
async fn logout() -> Redirect {
    // The actual logout happens on client side by clearing localStorage
    // This endpoint just redirects back
    Redirect::temporary("/")
}
