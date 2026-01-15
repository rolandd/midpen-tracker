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
