// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Strava API client for fetching and updating activities.
//!
//! Handles:
//! - Activity fetching with detailed polylines
//! - Activity description updates
//! - Token refresh when expired
//! - Rate limit detection (for Cloud Tasks retry)

use crate::error::AppError;
use serde::Deserialize;

/// Strava API client.
#[derive(Clone)]
pub struct StravaClient {
    http: reqwest::Client,
    base_url: String,
    client_id: String,
    client_secret: String,
}

impl StravaClient {
    /// Create a new Strava client with OAuth credentials.
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: "https://www.strava.com/api/v3".to_string(),
            client_id,
            client_secret,
        }
    }

    /// Get a detailed activity by ID.
    pub async fn get_activity(
        &self,
        access_token: &str,
        activity_id: u64,
    ) -> Result<StravaActivity, AppError> {
        let url = format!("{}/activities/{}", self.base_url, activity_id);
        self.get_json(&url, access_token).await
    }

    /// Update an activity's description.
    pub async fn update_activity_description(
        &self,
        access_token: &str,
        activity_id: u64,
        description: &str,
    ) -> Result<(), AppError> {
        let url = format!("{}/activities/{}", self.base_url, activity_id);

        let body = serde_json::json!({
            "description": description
        });

        let response = self
            .http
            .put(&url)
            .bearer_auth(access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::StravaApi(e.to_string()))?;

        self.check_response(response).await?;
        Ok(())
    }

    /// List activities for backfill (paginated).
    pub async fn list_activities(
        &self,
        access_token: &str,
        after: i64, // Unix timestamp
        page: u32,
        per_page: u32,
    ) -> Result<Vec<StravaActivitySummary>, AppError> {
        let url = format!("{}/athlete/activities", self.base_url);

        let response = self
            .http
            .get(&url)
            .bearer_auth(access_token)
            .query(&[
                ("after", after.to_string()),
                ("page", page.to_string()),
                ("per_page", per_page.to_string()),
            ])
            .send()
            .await
            .map_err(|e| AppError::StravaApi(e.to_string()))?;

        self.check_response_json(response).await
    }

    /// Refresh an expired access token.
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<TokenRefreshResponse, AppError> {
        let response = self
            .http
            .post("https://www.strava.com/oauth/token")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("refresh_token", refresh_token),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|e| AppError::StravaApi(format!("Token refresh request failed: {}", e)))?;

        self.check_response_json(response).await
    }

    /// Deauthorize the application for a user.
    ///
    /// POST https://www.strava.com/oauth/deauthorize
    /// Authorization: Bearer {access_token}
    ///
    /// This invalidates all access and refresh tokens for the user
    /// and removes the app from their Strava settings.
    pub async fn deauthorize(&self, access_token: &str) -> Result<(), AppError> {
        let response = self
            .http
            .post("https://www.strava.com/oauth/deauthorize")
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AppError::StravaApi(format!("Deauthorization request failed: {}", e)))?;

        self.check_response(response).await?;
        tracing::info!("Strava deauthorization successful");
        Ok(())
    }

    /// Get authenticated athlete profile.
    pub async fn get_athlete(&self, access_token: &str) -> Result<StravaAthlete, AppError> {
        let url = format!("{}/athlete", self.base_url);
        self.get_json(&url, access_token).await
    }

    /// Generic GET request with JSON response.
    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        url: &str,
        access_token: &str,
    ) -> Result<T, AppError> {
        let response = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| AppError::StravaApi(e.to_string()))?;

        self.check_response_json(response).await
    }

    /// Check response status and return error if not successful.
    async fn check_response(&self, response: reqwest::Response) -> Result<(), AppError> {
        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        // Rate limit - should trigger Cloud Tasks retry
        if status.as_u16() == 429 {
            tracing::warn!("Strava rate limit hit (429)");
            return Err(AppError::StravaApi(AppError::STRAVA_RATE_LIMIT.to_string()));
        }

        // Unauthorized - token may be expired
        if status.as_u16() == 401 {
            return Err(AppError::StravaApi(
                AppError::STRAVA_TOKEN_ERROR.to_string(),
            ));
        }

        Err(AppError::StravaApi(format!("HTTP {}: {}", status, body)))
    }

    /// Check response and parse JSON body.
    async fn check_response_json<T: for<'de> Deserialize<'de>>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, AppError> {
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            if status.as_u16() == 429 {
                tracing::warn!("Strava rate limit hit (429)");
                return Err(AppError::StravaApi(AppError::STRAVA_RATE_LIMIT.to_string()));
            }

            if status.as_u16() == 401 {
                return Err(AppError::StravaApi(
                    AppError::STRAVA_TOKEN_ERROR.to_string(),
                ));
            }

            return Err(AppError::StravaApi(format!("HTTP {}: {}", status, body)));
        }

        response
            .json()
            .await
            .map_err(|e| AppError::StravaApi(format!("JSON parse error: {}", e)))
    }
}

/// Token refresh response from Strava.
#[derive(Debug, Clone, Deserialize)]
pub struct TokenRefreshResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
}

/// Detailed Strava activity response.
#[derive(Debug, Clone, Deserialize)]
pub struct StravaActivity {
    pub id: u64,
    pub name: String,
    pub sport_type: String,
    pub start_date: String,
    pub distance: f64,
    pub description: Option<String>,
    pub device_name: Option<String>,
    pub map: StravaMap,
}

impl StravaActivity {
    /// Get the detailed polyline, falling back to summary if not available.
    pub fn get_polyline(&self) -> Option<&str> {
        self.map
            .polyline
            .as_deref()
            .or(self.map.summary_polyline.as_deref())
    }
}

/// Activity map data with polylines.
#[derive(Debug, Clone, Deserialize)]
pub struct StravaMap {
    pub polyline: Option<String>,
    pub summary_polyline: Option<String>,
}

/// Summary activity for list endpoints.
#[derive(Debug, Clone, Deserialize)]
pub struct StravaActivitySummary {
    pub id: u64,
    pub name: String,
    pub sport_type: String,
    pub start_date: String,
    pub distance: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// StravaService - High-level service with token management
// ─────────────────────────────────────────────────────────────────────────────

use crate::db::FirestoreDb;
use crate::models::{User, UserTokens};
use crate::services::KmsService;
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Margin before token expiration when we proactively refresh (5 minutes).
const TOKEN_REFRESH_MARGIN_SECS: i64 = 5 * 60;

/// Cached access token with expiry information.
#[derive(Clone)]
pub struct CachedToken {
    access_token: String,
    expires_at: DateTime<Utc>,
}

/// Shared token cache type for use in AppState.
pub type TokenCache = Arc<DashMap<u64, CachedToken>>;

/// Shared refresh locks type for use in AppState.
pub type RefreshLocks = Arc<DashMap<u64, Arc<Mutex<()>>>>;

/// High-level Strava service that manages token lifecycle and API calls.
///
/// This service encapsulates:
/// - Token retrieval and decryption from Firestore
/// - Automatic token refresh when expiring (with 5-minute margin)
/// - Re-encryption and storage of refreshed tokens
/// - In-memory token caching to reduce KMS calls
/// - Per-user locking to prevent duplicate refresh calls
/// - All Strava API calls
#[derive(Clone)]
pub struct StravaService {
    client: StravaClient,
    db: FirestoreDb,
    kms: KmsService,
    /// In-memory cache of decrypted access tokens (shared across requests).
    token_cache: TokenCache,
    /// Per-user mutex to serialize token refresh operations.
    refresh_locks: RefreshLocks,
}

impl StravaService {
    /// Create a new Strava service with shared token cache.
    ///
    /// The `token_cache` and `refresh_locks` should be shared across all
    /// `StravaService` instances to enable caching within a Cloud Run instance.
    pub fn new(
        client_id: String,
        client_secret: String,
        db: FirestoreDb,
        kms: KmsService,
        token_cache: TokenCache,
        refresh_locks: RefreshLocks,
    ) -> Self {
        Self {
            client: StravaClient::new(client_id, client_secret),
            db,
            kms,
            token_cache,
            refresh_locks,
        }
    }

    // ─── Token Management ────────────────────────────────────────────────────

    /// Get a valid (non-expired) access token for the given athlete.
    ///
    /// This method uses a multi-layer optimization strategy:
    /// 1. Check in-memory cache (fast path - no I/O)
    /// 2. Acquire per-user lock to prevent duplicate refresh calls
    /// 3. Re-check cache after lock (another task may have refreshed)
    /// 4. Fetch from Firestore and decrypt only the access token (lazy)
    /// 5. If token is valid, cache and return
    /// 6. If expired, decrypt refresh token and refresh with Strava
    /// 7. Handle cross-instance races via retry on invalid_grant
    pub async fn get_valid_access_token(&self, athlete_id: u64) -> Result<String, AppError> {
        let now = Utc::now();
        let margin = Duration::seconds(TOKEN_REFRESH_MARGIN_SECS);

        // ─────────────────────────────────────────────────────────────
        // STEP 1: Check cache (fast path - no I/O)
        // ─────────────────────────────────────────────────────────────
        if let Some(cached) = self.token_cache.get(&athlete_id) {
            if now + margin < cached.expires_at {
                // Cache hit, token still valid
                return Ok(cached.access_token.clone());
            }
            // Token expired or expiring soon - fall through to refresh
        }

        // ─────────────────────────────────────────────────────────────
        // STEP 2: Acquire per-user refresh lock
        // ─────────────────────────────────────────────────────────────
        // This ensures only one task per user performs the refresh.
        // Other tasks wait here until refresh completes.
        let lock = self
            .refresh_locks
            .entry(athlete_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();

        let _guard = lock.lock().await;

        // ─────────────────────────────────────────────────────────────
        // STEP 3: Re-check cache after acquiring lock (double-check)
        // ─────────────────────────────────────────────────────────────
        // Another task may have refreshed while we were waiting.
        if let Some(cached) = self.token_cache.get(&athlete_id) {
            if now + margin < cached.expires_at {
                // Another task already refreshed - use cached token
                return Ok(cached.access_token.clone());
            }
        }

        // ─────────────────────────────────────────────────────────────
        // STEP 4: Fetch from Firestore and decrypt (LAZY - access only)
        // ─────────────────────────────────────────────────────────────
        let tokens = self
            .db
            .get_tokens(athlete_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Tokens for athlete {}", athlete_id)))?;

        // LAZY DECRYPTION: Only decrypt access token first
        let access_token = self
            .kms
            .decrypt_or_fallback(
                &tokens.access_token_encrypted,
                athlete_id.to_string().as_bytes(),
            )
            .await?;

        let expires_at = DateTime::parse_from_rfc3339(&tokens.expires_at)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to parse expiry: {}", e)))?
            .with_timezone(&Utc);

        // ─────────────────────────────────────────────────────────────
        // STEP 5: Check if refresh is needed
        // ─────────────────────────────────────────────────────────────
        if now + margin < expires_at {
            // Token is still valid - cache and return
            self.token_cache.insert(
                athlete_id,
                CachedToken {
                    access_token: access_token.clone(),
                    expires_at,
                },
            );
            return Ok(access_token);
        }

        // ─────────────────────────────────────────────────────────────
        // STEP 6: Token expired - decrypt refresh token and refresh
        //         with cross-instance race handling
        // ─────────────────────────────────────────────────────────────
        tracing::info!(athlete_id, "Access token expired, refreshing");

        let refresh_token = self
            .kms
            .decrypt_or_fallback(
                &tokens.refresh_token_encrypted,
                athlete_id.to_string().as_bytes(),
            )
            .await?;

        // Handle cross-instance race: if another Cloud Run instance already
        // refreshed the token, Strava will reject our old refresh token.
        // In that case, fetch the winner's tokens from Firestore.
        let new_tokens = match self.client.refresh_token(&refresh_token).await {
            Ok(t) => t,
            Err(AppError::StravaApi(ref msg)) if msg.contains("invalid_grant") => {
                tracing::info!(
                    athlete_id,
                    "Refresh token race detected - another instance won, fetching their tokens"
                );
                return self.fetch_and_cache_from_db(athlete_id).await;
            }
            Err(e) => return Err(e),
        };

        // ─────────────────────────────────────────────────────────────
        // STEP 7: Encrypt and store new tokens
        // ─────────────────────────────────────────────────────────────
        let (new_enc_access, new_enc_refresh) = crate::services::kms::encrypt_tokens(
            &self.kms,
            &new_tokens.access_token,
            &new_tokens.refresh_token,
            athlete_id,
        )
        .await?;

        let new_expires_at = DateTime::from_timestamp(new_tokens.expires_at, 0).unwrap_or_default();

        let updated_tokens = UserTokens {
            access_token_encrypted: new_enc_access,
            refresh_token_encrypted: new_enc_refresh,
            expires_at: new_expires_at.to_rfc3339(),
            scopes: tokens.scopes.clone(),
        };

        self.db.set_tokens(athlete_id, &updated_tokens).await?;

        // ─────────────────────────────────────────────────────────────
        // STEP 8: Update cache with new token
        // ─────────────────────────────────────────────────────────────
        self.token_cache.insert(
            athlete_id,
            CachedToken {
                access_token: new_tokens.access_token.clone(),
                expires_at: new_expires_at,
            },
        );

        tracing::info!(athlete_id, "Token refreshed and cached");
        Ok(new_tokens.access_token)
    }

    /// Fetch fresh tokens from Firestore (after cross-instance race) and cache.
    ///
    /// Used when we detect another Cloud Run instance won the refresh race.
    async fn fetch_and_cache_from_db(&self, athlete_id: u64) -> Result<String, AppError> {
        let tokens = self
            .db
            .get_tokens(athlete_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Tokens for athlete {}", athlete_id)))?;

        let access_token = self
            .kms
            .decrypt_or_fallback(
                &tokens.access_token_encrypted,
                athlete_id.to_string().as_bytes(),
            )
            .await?;

        let expires_at = DateTime::parse_from_rfc3339(&tokens.expires_at)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to parse expiry: {}", e)))?
            .with_timezone(&Utc);

        self.token_cache.insert(
            athlete_id,
            CachedToken {
                access_token: access_token.clone(),
                expires_at,
            },
        );

        Ok(access_token)
    }

    // ─── OAuth Callback Handling ─────────────────────────────────────────────

    /// Handle OAuth callback: exchange code for tokens, store user and tokens.
    ///
    /// Returns the athlete ID and plaintext access token (for immediate use).
    pub async fn handle_oauth_callback(&self, code: &str) -> Result<OAuthResult, AppError> {
        // Exchange code for tokens
        let token_response = self.exchange_code(code).await?;

        let athlete_id = token_response.athlete.id;
        let now = chrono::Utc::now().to_rfc3339();

        // Store user profile
        let user = User {
            strava_athlete_id: athlete_id,
            email: None,
            firstname: token_response.athlete.firstname.clone(),
            lastname: token_response.athlete.lastname.clone(),
            profile_picture: token_response.athlete.profile.clone(),
            created_at: now.clone(),
            last_active: now.clone(),
            deletion_requested_at: None,
        };

        if let Err(e) = self.db.upsert_user(&user).await {
            tracing::warn!(error = %e, "Failed to store user profile, continuing anyway");
        }

        // Encrypt and store tokens
        let expires_at =
            chrono::DateTime::<chrono::Utc>::from_timestamp(token_response.expires_at, 0)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| now.clone());

        let (enc_access, enc_refresh) = crate::services::kms::encrypt_tokens(
            &self.kms,
            &token_response.access_token,
            &token_response.refresh_token,
            athlete_id,
        )
        .await?;

        let tokens = UserTokens {
            access_token_encrypted: enc_access,
            refresh_token_encrypted: enc_refresh,
            expires_at,
            scopes: vec![
                "activity:read_all".to_string(),
                "activity:write".to_string(),
            ],
        };

        self.db.set_tokens(athlete_id, &tokens).await?;

        tracing::info!(
            athlete_id,
            firstname = %token_response.athlete.firstname,
            "OAuth callback handled, user and tokens stored"
        );

        Ok(OAuthResult {
            athlete_id,
            firstname: token_response.athlete.firstname,
            lastname: token_response.athlete.lastname,
            access_token: token_response.access_token,
        })
    }

    /// Exchange authorization code for tokens (internal helper).
    async fn exchange_code(&self, code: &str) -> Result<StravaTokenExchangeResponse, AppError> {
        let response = self
            .client
            .http
            .post("https://www.strava.com/oauth/token")
            .form(&[
                ("client_id", self.client.client_id.as_str()),
                ("client_secret", self.client.client_secret.as_str()),
                ("code", code),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .map_err(|e| AppError::StravaApi(format!("Token exchange failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!(status = %status, body = %body, "Strava token exchange failed");
            return Err(AppError::StravaApi(format!(
                "Token exchange failed with status {}",
                status
            )));
        }

        response
            .json()
            .await
            .map_err(|e| AppError::StravaApi(format!("Failed to parse token response: {}", e)))
    }

    // ─── API Wrappers ────────────────────────────────────────────────────────

    /// Get a detailed activity by ID.
    pub async fn get_activity(
        &self,
        athlete_id: u64,
        activity_id: u64,
    ) -> Result<StravaActivity, AppError> {
        let access_token = self.get_valid_access_token(athlete_id).await?;
        self.client.get_activity(&access_token, activity_id).await
    }

    /// List activities for backfill (paginated).
    pub async fn list_activities(
        &self,
        athlete_id: u64,
        after: i64,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<StravaActivitySummary>, AppError> {
        let access_token = self.get_valid_access_token(athlete_id).await?;
        self.client
            .list_activities(&access_token, after, page, per_page)
            .await
    }

    /// Update an activity's description.
    pub async fn update_activity_description(
        &self,
        athlete_id: u64,
        activity_id: u64,
        description: &str,
    ) -> Result<(), AppError> {
        let access_token = self.get_valid_access_token(athlete_id).await?;
        self.client
            .update_activity_description(&access_token, activity_id, description)
            .await
    }

    /// Deauthorize with a specific token.
    pub async fn deauthorize_with_token(&self, access_token: &str) -> Result<(), AppError> {
        self.client.deauthorize(access_token).await
    }

    /// Verify that the user's token is still valid by making a request to Strava.
    /// This bypasses the cache's assumption of validity based on timestamp.
    /// Returns Ok(true) if active, Ok(false) if revoked/expired, Err on other errors.
    pub async fn verify_token_active(&self, athlete_id: u64) -> Result<bool, AppError> {
        // Attempt to get a candidate token (refreshing if needed)
        let access_token = match self.get_valid_access_token(athlete_id).await {
            Ok(t) => t,
            // If we can't get a token due to Auth/Refresh failure, it's definitely not active
            Err(e) if e.is_strava_token_error() => return Ok(false),
            Err(e) => return Err(e),
        };

        // Validate against Strava API using a lightweight call
        match self.client.get_athlete(&access_token).await {
            Ok(_) => Ok(true),
            Err(e) if e.is_strava_token_error() => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Revoke local tokens from DB and return a valid access token for final cleanup.
    ///
    /// This method:
    /// 1. Reads tokens from DB.
    /// 2. Deletes tokens from DB immediately (to block concurrent processing).
    /// 3. Decrypts and checks expiration.
    /// 4. Refreshes token with Strava if needed (in-memory only).
    /// 5. Returns the valid access token.
    pub async fn revoke_local_tokens(&self, athlete_id: u64) -> Result<Option<String>, AppError> {
        // 1. Get tokens
        let tokens_opt = self.db.get_tokens(athlete_id).await?;
        let tokens = match tokens_opt {
            Some(t) => t,
            None => return Ok(None),
        };

        // 2. Delete tokens immediately
        self.db.delete_tokens(athlete_id).await?;

        // 3. Decrypt
        let (mut access_token, refresh_token) = match crate::services::kms::decrypt_tokens(
            &self.kms,
            &tokens.access_token_encrypted,
            &tokens.refresh_token_encrypted,
            athlete_id,
        )
        .await
        {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    athlete_id,
                    "Failed to decrypt tokens (skipping deauth)"
                );
                return Ok(None);
            }
        };

        // 4. Check expiration & Refresh in-memory
        let expires_at = chrono::DateTime::parse_from_rfc3339(&tokens.expires_at)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        let now = chrono::Utc::now();
        let margin = chrono::Duration::seconds(TOKEN_REFRESH_MARGIN_SECS);

        if now + margin >= expires_at {
            tracing::info!(
                athlete_id,
                "Token expired during deletion, refreshing in-memory"
            );
            match self.client.refresh_token(&refresh_token).await {
                Ok(new_tokens) => {
                    access_token = new_tokens.access_token;
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        athlete_id,
                        "Failed to refresh token for deauth (attempting with old token)"
                    );
                }
            }
        }

        Ok(Some(access_token))
    }
}

/// Token exchange response from Strava OAuth (includes athlete info).
#[derive(Debug, Clone, Deserialize)]
struct StravaTokenExchangeResponse {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    athlete: StravaAthlete,
}

/// Athlete info from OAuth token exchange.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StravaAthlete {
    pub id: u64,
    firstname: String,
    lastname: String,
    profile: Option<String>,
}

/// Result of handling OAuth callback.
#[derive(Debug, Clone)]
pub struct OAuthResult {
    pub athlete_id: u64,
    pub firstname: String,
    pub lastname: String,
    pub access_token: String,
}
