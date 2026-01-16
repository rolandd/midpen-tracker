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
            return Err(AppError::StravaApi("Rate limit exceeded".to_string()));
        }

        // Unauthorized - token may be expired
        if status.as_u16() == 401 {
            return Err(AppError::StravaApi("Token expired or invalid".to_string()));
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
                return Err(AppError::StravaApi("Rate limit exceeded".to_string()));
            }

            if status.as_u16() == 401 {
                return Err(AppError::StravaApi("Token expired or invalid".to_string()));
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
use serde::Serialize;

/// Margin before token expiration when we proactively refresh (5 minutes).
const TOKEN_REFRESH_MARGIN_SECS: i64 = 5 * 60;

/// High-level Strava service that manages token lifecycle and API calls.
///
/// This service encapsulates:
/// - Token retrieval and decryption from Firestore
/// - Automatic token refresh when expiring (with 5-minute margin)
/// - Re-encryption and storage of refreshed tokens
/// - All Strava API calls
#[derive(Clone)]
pub struct StravaService {
    client: StravaClient,
    db: FirestoreDb,
    kms: KmsService,
}

impl StravaService {
    /// Create a new Strava service.
    pub fn new(client_id: String, client_secret: String, db: FirestoreDb, kms: KmsService) -> Self {
        Self {
            client: StravaClient::new(client_id, client_secret),
            db,
            kms,
        }
    }

    // ─── Token Management ────────────────────────────────────────────────────

    /// Get a valid (non-expired) access token for the given athlete.
    ///
    /// This method:
    /// 1. Retrieves encrypted tokens from Firestore
    /// 2. Decrypts them using KMS
    /// 3. Checks expiration (with 5-minute margin)
    /// 4. Refreshes if needed and updates Firestore
    /// 5. Returns the valid access token
    pub async fn get_valid_access_token(&self, athlete_id: u64) -> Result<String, AppError> {
        // 1. Get encrypted tokens from DB
        let tokens = self
            .db
            .get_tokens(athlete_id)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Tokens for athlete {}", athlete_id)))?;

        // 2. Decrypt tokens
        let (access_token, refresh_token) = crate::services::kms::decrypt_tokens(
            &self.kms,
            &tokens.access_token_encrypted,
            &tokens.refresh_token_encrypted,
        )
        .await?;

        // 3. Check expiration
        let expires_at = chrono::DateTime::parse_from_rfc3339(&tokens.expires_at)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Failed to parse expiry: {}", e)))?
            .with_timezone(&chrono::Utc);

        let now = chrono::Utc::now();
        let margin = chrono::Duration::seconds(TOKEN_REFRESH_MARGIN_SECS);

        // 4. Refresh if expiring within margin
        if now + margin >= expires_at {
            tracing::info!(athlete_id, "Access token expiring soon, refreshing");
            return self
                .refresh_and_store_token(athlete_id, &refresh_token, &tokens)
                .await;
        }

        // 5. Token is still valid
        Ok(access_token)
    }

    /// Refresh the access token and store the new tokens in Firestore.
    async fn refresh_and_store_token(
        &self,
        athlete_id: u64,
        refresh_token: &str,
        existing_tokens: &UserTokens,
    ) -> Result<String, AppError> {
        // Call Strava to refresh
        let new_tokens = self.client.refresh_token(refresh_token).await?;

        // Encrypt new tokens
        let (new_enc_access, new_enc_refresh) = crate::services::kms::encrypt_tokens(
            &self.kms,
            &new_tokens.access_token,
            &new_tokens.refresh_token,
        )
        .await?;

        // Build updated token document
        let updated_tokens = UserTokens {
            access_token_encrypted: new_enc_access,
            refresh_token_encrypted: new_enc_refresh,
            expires_at: chrono::DateTime::from_timestamp(new_tokens.expires_at, 0)
                .unwrap_or_default()
                .to_rfc3339(),
            scopes: existing_tokens.scopes.clone(),
        };

        // Store in Firestore
        self.db.set_tokens(athlete_id, &updated_tokens).await?;

        tracing::info!(athlete_id, "Token refreshed and stored");
        Ok(new_tokens.access_token)
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
struct StravaAthlete {
    id: u64,
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
