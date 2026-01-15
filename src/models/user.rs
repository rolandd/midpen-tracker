//! User model for storage and API.

use serde::{Deserialize, Serialize};

/// User profile stored in Firestore.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    /// Strava athlete ID (also used as document ID)
    pub strava_athlete_id: u64,
    /// Email address (may be None if not shared)
    pub email: Option<String>,
    /// First name
    pub firstname: String,
    /// Last name
    pub lastname: String,
    /// Profile picture URL
    pub profile_picture: Option<String>,
    /// When user first connected
    pub created_at: String,
    /// Last activity timestamp
    pub last_active: String,
}

/// User's OAuth tokens (encrypted in Firestore).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserTokens {
    /// Encrypted access token (base64)
    pub access_token_encrypted: String,
    /// Encrypted refresh token (base64)
    pub refresh_token_encrypted: String,
    /// When the access token expires (ISO 8601)
    pub expires_at: String,
    /// Granted OAuth scopes
    pub scopes: Vec<String>,
}
