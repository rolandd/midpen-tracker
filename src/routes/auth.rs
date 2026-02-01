// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Strava OAuth authentication routes.

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect}, // Added IntoResponse
    routing::{get, post},
    Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite}; // Added axum-extra
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use ring::rand::SecureRandom;
use serde::Deserialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use time; // Added time import for Cookie max_age

use crate::error::{AppError, Result};
use crate::services::strava::StravaService;
use crate::services::KmsService;
use crate::AppState;

/// Cookie name for the client-side login hint (used to prevent FOUC).
const HINT_COOKIE_NAME: &str = "midpen_logged_in";

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/auth/strava", get(auth_start))
        .route("/auth/strava/callback", get(auth_callback))
        .route("/auth/logout", post(logout))
}

use crate::middleware::auth::create_jwt;

/// Query parameters for starting OAuth flow.
#[derive(Deserialize)]
pub struct AuthStartParams {
    /// Frontend URL to redirect back to after OAuth completes.
    /// If not provided, uses FRONTEND_URL env var.
    #[serde(default)]
    redirect_uri: Option<String>,
}

/// Start OAuth flow - redirect to Strava authorization.
// ... imports ...
use hmac::{Hmac, Mac};
use sha2::Sha256;

// Type alias for HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

/// Start OAuth flow - redirect to Strava authorization.
async fn auth_start(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Query(params): Query<AuthStartParams>,
    headers: axum::http::HeaderMap,
) -> Result<impl IntoResponse> {
    // Get the frontend URL from query param or fall back to config
    let frontend_url = params
        .redirect_uri
        .unwrap_or_else(|| state.config.frontend_url.clone());

    // Generate random nonce (16 bytes)
    let mut nonce_bytes = [0u8; 16];
    let rng = ring::rand::SystemRandom::new();
    rng.fill(&mut nonce_bytes).map_err(|e| {
        AppError::Internal(anyhow::anyhow!("Random number generation failed: {}", e))
    })?;
    let nonce_hex = hex::encode(nonce_bytes);

    // Encode frontend URL + timestamp + nonce in state
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("System time error: {}", e)))?
        .as_millis();

    // Create the data payload: "frontend_url|timestamp_hex|nonce_hex"
    let state_payload = format!("{}|{:x}|{}", frontend_url, timestamp, nonce_hex);

    // Sign the payload
    let mut mac = HmacSha256::new_from_slice(&state.config.oauth_state_key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("HMAC init failed: {}", e)))?;
    mac.update(state_payload.as_bytes());
    let signature = mac.finalize().into_bytes();

    // Combine payload + signature: "payload|signature_hex"
    let signed_state = format!("{}|{}", state_payload, hex::encode(signature));

    // Base64 encode the whole thing for the URL
    let oauth_state = URL_SAFE_NO_PAD.encode(signed_state.as_bytes());

    // Create HttpOnly cookie for the nonce
    // Path limited to callback to reduce leakage
    let mut cookie = Cookie::new("midpen_oauth_nonce", nonce_hex);
    cookie.set_http_only(true);
    let is_production = state.config.frontend_url.contains("https");
    cookie.set_secure(is_production);
    cookie.set_path("/auth/strava/callback");
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(time::Duration::minutes(15)); // Matches state expiry

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

    Ok((jar.add(cookie), Redirect::temporary(&auth_url)))
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: String,
    state: String,
    #[serde(default)]
    error: Option<String>,
}

/// OAuth callback - exchange code for tokens, create session.
async fn auth_callback(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Query(params): Query<CallbackParams>,
) -> Result<impl IntoResponse> {
    // Get nonce from cookie
    let nonce_cookie = jar.get("midpen_oauth_nonce").map(|c| c.value().to_string());

    // Cleanup nonce cookie immediately
    let mut cleanup_cookie = Cookie::new("midpen_oauth_nonce", "");
    cleanup_cookie.set_path("/auth/strava/callback");
    cleanup_cookie.set_max_age(time::Duration::seconds(0));
    let jar = jar.add(cleanup_cookie);

    // Decode and verify frontend URL from state parameter
    let frontend_url = verify_and_decode_state(
        &params.state,
        &state.config.oauth_state_key,
        nonce_cookie.as_deref(),
    )
    .ok_or_else(|| {
        tracing::warn!("Invalid or tampered state parameter, aborting authentication");
        AppError::Unauthorized
    })?;

    // Check for OAuth errors
    if let Some(error) = params.error {
        tracing::warn!(error = %error, "OAuth error from Strava");
        let redirect = format!("{}?error={}", frontend_url, error);
        return Ok((jar, Redirect::temporary(&redirect))); // Return jar + redirect
    }

    tracing::info!("Exchanging authorization code for tokens");

    // Initialize KMS service
    let kms = KmsService::new(
        &state.config.gcp_project_id,
        &state.config.gcp_region,
        "token-encryption",
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Failed to initialize KMS service");
        e
    })?;

    // Create StravaService for OAuth handling
    let strava_service = StravaService::new(
        state.config.strava_client_id.clone(),
        state.config.strava_client_secret.clone(),
        state.db.clone(),
        kms,
        state.token_cache.clone(),
        state.refresh_locks.clone(),
    );

    // Handle OAuth callback: exchange code, store user and tokens
    let oauth_result = strava_service.handle_oauth_callback(&params.code).await?;

    tracing::info!(
        athlete_id = oauth_result.athlete_id,
        firstname = %oauth_result.firstname,
        "OAuth successful, user and tokens stored"
    );

    // Trigger backfill for activities since 2025-01-01
    let backfill_result = trigger_backfill(
        &state,
        &strava_service,
        oauth_result.athlete_id,
        &state.config.api_url,
    )
    .await;

    if let Err(e) = backfill_result {
        tracing::warn!(error = %e, "Failed to trigger backfill, continuing anyway");
    }

    // Create JWT session token
    let jwt = create_jwt(oauth_result.athlete_id, &state.config.jwt_signing_key)
        .map_err(|e| AppError::Internal(anyhow::anyhow!("JWT creation failed: {}", e)))?;

    // Redirect to frontend with token
    // Create HttpOnly cookie
    let mut cookie = Cookie::new("midpen_token", jwt);
    cookie.set_http_only(true);
    let is_production = state.config.frontend_url.contains("https");
    cookie.set_secure(is_production);
    cookie.set_path("/");
    cookie.set_same_site(SameSite::Lax);
    // Set max age to 30 days (same as JWT exp)
    cookie.set_max_age(time::Duration::days(30));

    // Create hint cookie (not HttpOnly) for client-side detection
    // This cookie needs a domain that covers both API and frontend subdomains
    let mut hint_cookie = Cookie::new(HINT_COOKIE_NAME, "1");
    hint_cookie.set_http_only(false); // JS can read this
    hint_cookie.set_secure(is_production);
    hint_cookie.set_path("/");
    hint_cookie.set_same_site(SameSite::Lax);
    hint_cookie.set_max_age(time::Duration::days(30));

    // Set domain for cross-subdomain access (e.g., .rolandd.dev)
    // This allows the frontend (midpen-tracker.rolandd.dev) to read cookies
    // set by the API (midpen-tracker-api.rolandd.dev)
    if let Some(domain) = extract_cookie_domain(&frontend_url) {
        hint_cookie.set_domain(domain);
    }

    // Redirect to dashboard (no token in URL)
    let redirect_url = format!("{}/dashboard", frontend_url);

    Ok((
        jar.add(cookie).add(hint_cookie),
        Redirect::temporary(&redirect_url),
    ))
}

/// Extract a cookie domain from a URL for cross-subdomain sharing.
/// Returns the root domain with a leading dot (e.g., ".rolandd.dev").
/// Returns None for localhost or invalid URLs.
fn extract_cookie_domain(url: &str) -> Option<String> {
    use std::net::IpAddr;

    // Parse the URL to get the host
    let host = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;

    // Remove any path/port
    let host = host.split('/').next()?;
    let host = host.split(':').next()?;

    // Check for localhost (literal string)
    if host == "localhost" {
        return None;
    }

    // Check for loopback IPs (127.x.x.x, ::1, etc.)
    if let Ok(ip) = host.parse::<IpAddr>() {
        if ip.is_loopback() {
            return None;
        }
    }

    // Extract root domain (last two parts: domain.tld)
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() >= 2 {
        // e.g., ["midpen-tracker", "rolandd", "dev"] -> ".rolandd.dev"
        let root_domain = parts[parts.len() - 2..].join(".");
        Some(format!(".{}", root_domain))
    } else {
        None
    }
}

/// Trigger backfill for activities since 2025-01-01.
/// Only fetches first page at login, then queues a continue-backfill task
/// for subsequent pages to spread Strava API calls over time.
async fn trigger_backfill(
    state: &Arc<AppState>,
    strava: &StravaService,
    athlete_id: u64,
    service_url: &str,
) -> Result<()> {
    use crate::models::UserStats;
    use crate::services::tasks::ContinueBackfillPayload;

    // Backfill activities since 2025-01-01
    let after_timestamp = chrono::NaiveDate::from_ymd_opt(2025, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp();

    // Fetch ONLY the first page at login (100 activities max)
    let per_page = 100u32;
    let activities = strava
        .list_activities(athlete_id, after_timestamp, 1, per_page)
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

/// Verify HMAC signature and decode the frontend URL from the OAuth state parameter.
fn verify_and_decode_state(
    state: &str,
    secret: &[u8],
    expected_nonce: Option<&str>,
) -> Option<String> {
    let bytes = URL_SAFE_NO_PAD.decode(state).ok()?;
    let state_str = String::from_utf8(bytes).ok()?;

    // Format is "frontend_url|timestamp_hex|nonce_hex|signature_hex"
    let parts: Vec<&str> = state_str.splitn(4, '|').collect();
    if parts.len() != 4 {
        tracing::warn!("Invalid OAuth state format");
        return None;
    }

    let frontend_url = parts[0];
    let timestamp_hex = parts[1];
    let nonce_hex = parts[2];
    let signature_hex = parts[3];

    // Reconstruct payload and verify signature
    let payload = format!("{}|{}|{}", frontend_url, timestamp_hex, nonce_hex);

    let mut mac = HmacSha256::new_from_slice(secret).ok()?;
    mac.update(payload.as_bytes());

    let expected_signature = hex::encode(mac.finalize().into_bytes());

    if signature_hex != expected_signature {
        tracing::warn!("OAuth state signature mismatch! Potential tampering.");
        return None;
    }

    // Verify nonce
    if let Some(expected) = expected_nonce {
        if nonce_hex != expected {
            tracing::warn!("OAuth state nonce mismatch! CSRF attack?");
            return None;
        }
    } else {
        tracing::warn!("Missing nonce cookie for verification");
        return None;
    }

    // Verify timestamp
    let timestamp_millis = u128::from_str_radix(timestamp_hex, 16).ok()?;
    let now_millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis();

    // 15 minutes = 900,000 ms
    const MAX_AGE_MS: u128 = 15 * 60 * 1000;
    // 5 minutes skew = 300,000 ms
    const FUTURE_SKEW_MS: u128 = 5 * 60 * 1000;

    if timestamp_millis < now_millis.saturating_sub(MAX_AGE_MS) {
        tracing::warn!(
            timestamp = timestamp_millis,
            now = now_millis,
            "OAuth state expired"
        );
        return None;
    }

    if timestamp_millis > now_millis.saturating_add(FUTURE_SKEW_MS) {
        tracing::warn!(
            timestamp = timestamp_millis,
            now = now_millis,
            "OAuth state timestamp in future"
        );
        return None;
    }

    Some(frontend_url.to_string())
}

/// Logout - clear the auth cookie.
async fn logout(jar: CookieJar) -> (CookieJar, axum::http::StatusCode) {
    // Cookie removal must match the same attributes as when it was set
    // (path, secure, httponly, samesite) for browser to recognize it
    let cookie = Cookie::build("midpen_token")
        .path("/")
        .http_only(true)
        .secure(true) // Must match what was set during login
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(0))
        .build();

    let hint_cookie = Cookie::build(HINT_COOKIE_NAME)
        .path("/")
        .http_only(false)
        .secure(true)
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(0))
        .build();

    // Also clear the nonce cookie just in case
    let nonce_cookie = Cookie::build("midpen_oauth_nonce")
        .path("/auth/strava/callback")
        .max_age(time::Duration::seconds(0))
        .build();

    (
        jar.remove(cookie).remove(hint_cookie).remove(nonce_cookie),
        axum::http::StatusCode::NO_CONTENT,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to generate a valid state for testing
    fn generate_test_state(
        frontend_url: &str,
        timestamp: u128,
        nonce: &str,
        secret: &[u8],
    ) -> String {
        let payload = format!("{}|{:x}|{}", frontend_url, timestamp, nonce);
        let mut mac = HmacSha256::new_from_slice(secret).unwrap();
        mac.update(payload.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let state_data = format!("{}|{}", payload, signature);
        URL_SAFE_NO_PAD.encode(state_data.as_bytes())
    }

    #[test]
    fn test_verify_and_decode_state_success() {
        let secret = b"secret_key";
        let frontend_url = "https://example.com";
        let nonce = "test_nonce_123";
        // Use current time
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let encoded_state = generate_test_state(frontend_url, timestamp, nonce, secret);
        let result = verify_and_decode_state(&encoded_state, secret, Some(nonce));
        assert_eq!(result, Some(frontend_url.to_string()));
    }

    #[test]
    fn test_verify_and_decode_state_expired() {
        let secret = b"secret_key";
        let frontend_url = "https://example.com";
        let nonce = "nonce";
        // 16 minutes ago
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            - 16 * 60 * 1000;

        let encoded_state = generate_test_state(frontend_url, timestamp, nonce, secret);
        let result = verify_and_decode_state(&encoded_state, secret, Some(nonce));
        assert_eq!(result, None);
    }

    #[test]
    fn test_verify_and_decode_state_future() {
        let secret = b"secret_key";
        let frontend_url = "https://example.com";
        let nonce = "nonce";
        // 6 minutes in future
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            + 6 * 60 * 1000;

        let encoded_state = generate_test_state(frontend_url, timestamp, nonce, secret);
        let result = verify_and_decode_state(&encoded_state, secret, Some(nonce));
        assert_eq!(result, None);
    }

    #[test]
    fn test_verify_and_decode_state_skew_allowed() {
        let secret = b"secret_key";
        let frontend_url = "https://example.com";
        let nonce = "nonce";
        // 4 minutes in future (allowed)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            + 4 * 60 * 1000;

        let encoded_state = generate_test_state(frontend_url, timestamp, nonce, secret);
        let result = verify_and_decode_state(&encoded_state, secret, Some(nonce));
        assert_eq!(result, Some(frontend_url.to_string()));
    }

    #[test]
    fn test_verify_and_decode_state_invalid_signature() {
        let secret = b"secret_key";
        let frontend_url = "https://example.com";
        let nonce = "nonce";
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let payload = format!("{}|{:x}|{}", frontend_url, timestamp, nonce);
        let signature = "invalid_signature";

        let state_data = format!("{}|{}", payload, signature);
        let encoded_state = URL_SAFE_NO_PAD.encode(state_data.as_bytes());

        let result = verify_and_decode_state(&encoded_state, secret, Some(nonce));
        assert_eq!(result, None);
    }

    #[test]
    fn test_verify_and_decode_state_wrong_secret() {
        let secret = b"secret_key";
        let wrong_secret = b"wrong_key";
        let frontend_url = "https://example.com";
        let nonce = "nonce";
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let encoded_state = generate_test_state(frontend_url, timestamp, nonce, secret);
        let result = verify_and_decode_state(&encoded_state, wrong_secret, Some(nonce));
        assert_eq!(result, None);
    }

    #[test]
    fn test_verify_and_decode_state_nonce_mismatch() {
        let secret = b"secret_key";
        let frontend_url = "https://example.com";
        let nonce = "correct_nonce";
        let wrong_nonce = "wrong_nonce";
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let encoded_state = generate_test_state(frontend_url, timestamp, nonce, secret);
        let result = verify_and_decode_state(&encoded_state, secret, Some(wrong_nonce));
        assert_eq!(result, None);
    }

    #[test]
    fn test_verify_and_decode_state_missing_nonce() {
        let secret = b"secret_key";
        let frontend_url = "https://example.com";
        let nonce = "correct_nonce";
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();

        let encoded_state = generate_test_state(frontend_url, timestamp, nonce, secret);
        let result = verify_and_decode_state(&encoded_state, secret, None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_verify_and_decode_state_malformed() {
        let secret = b"secret_key";
        let encoded_state = URL_SAFE_NO_PAD.encode("invalid|format");
        let result = verify_and_decode_state(&encoded_state, secret, None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_cookie_domain_subdomain() {
        let url = "https://midpen-tracker.rolandd.dev";
        assert_eq!(extract_cookie_domain(url), Some(".rolandd.dev".to_string()));
    }

    #[test]
    fn test_extract_cookie_domain_deep_subdomain() {
        let url = "https://api.midpen-tracker.rolandd.dev";
        assert_eq!(extract_cookie_domain(url), Some(".rolandd.dev".to_string()));
    }

    #[test]
    fn test_extract_cookie_domain_localhost() {
        let url = "http://localhost:5173";
        assert_eq!(extract_cookie_domain(url), None);
    }

    #[test]
    fn test_extract_cookie_domain_127() {
        let url = "http://127.0.0.1:8080";
        assert_eq!(extract_cookie_domain(url), None);
    }

    #[test]
    fn test_extract_cookie_domain_ipv6_loopback() {
        let url = "http://[::1]:8080";
        // IPv6 in URLs is bracketed, but we strip port first then brackets
        // This may not parse correctly - let's verify behavior
        assert_eq!(extract_cookie_domain(url), None);
    }

    #[test]
    fn test_extract_cookie_domain_simple() {
        let url = "https://example.com";
        assert_eq!(extract_cookie_domain(url), Some(".example.com".to_string()));
    }
}
