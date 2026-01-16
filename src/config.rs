//! Application configuration loaded from environment variables and Secret Manager.
//!
//! Secrets are fetched once at startup and cached in memory to minimize
//! Secret Manager API costs.

use std::env;

/// Application configuration, loaded once at startup.
/// Secrets are cached in memory after being fetched from Secret Manager.
#[derive(Debug, Clone)]
pub struct Config {
    // --- Environment Variables (non-sensitive) ---
    /// Strava OAuth client ID (public)
    pub strava_client_id: String,
    /// Frontend URL for OAuth redirects
    pub frontend_url: String,
    /// GCP project ID
    pub gcp_project_id: String,
    /// Server port
    pub port: u16,

    // --- Secrets (cached from Secret Manager) ---
    /// Strava OAuth client secret
    pub strava_client_secret: String,
    /// JWT signing key for session tokens (raw bytes)
    pub jwt_signing_key: Vec<u8>,
    /// Webhook verification token
    pub webhook_verify_token: String,
}

impl Default for Config {
    /// Default config for testing only.
    fn default() -> Self {
        Self {
            strava_client_id: "test_client_id".to_string(),
            frontend_url: "http://localhost:5173".to_string(),
            gcp_project_id: "test-project".to_string(),
            port: 8080,
            strava_client_secret: "test_secret".to_string(),
            jwt_signing_key: b"test_jwt_key_32_bytes_minimum!!".to_vec(),
            webhook_verify_token: "test_verify_token".to_string(),
        }
    }
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// For local development, secrets can be set via environment variables.
    /// In production, use `load_with_secrets()` to fetch from Secret Manager.
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok(); // Load .env file if present

        Ok(Self {
            // Non-sensitive config from env
            strava_client_id: env::var("STRAVA_CLIENT_ID")
                .map_err(|_| ConfigError::Missing("STRAVA_CLIENT_ID"))?,
            frontend_url: env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
            gcp_project_id: env::var("GCP_PROJECT_ID").unwrap_or_else(|_| "local-dev".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),

            // Secrets - from env for local dev, Secret Manager in prod
            strava_client_secret: env::var("STRAVA_CLIENT_SECRET")
                .map(|v| v.trim().to_string())
                .map_err(|_| ConfigError::Missing("STRAVA_CLIENT_SECRET"))?,
            jwt_signing_key: env::var("JWT_SIGNING_KEY")
                .map_err(|_| ConfigError::Missing("JWT_SIGNING_KEY"))?
                .into_bytes(),
            webhook_verify_token: env::var("WEBHOOK_VERIFY_TOKEN")
                .map(|v| v.trim().to_string())
                .map_err(|_| ConfigError::Missing("WEBHOOK_VERIFY_TOKEN"))?,
        })
    }

    /// Load configuration with secrets from Google Secret Manager.
    ///
    /// This is the recommended method for production deployments.
    /// Secrets are fetched once at startup and cached in memory.
    #[cfg(feature = "gcp")]
    pub async fn load_with_secrets() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        let project_id =
            env::var("GCP_PROJECT_ID").map_err(|_| ConfigError::Missing("GCP_PROJECT_ID"))?;

        tracing::info!(project = %project_id, "Fetching secrets from Secret Manager");

        // Fetch all secrets in parallel at startup
        let (client_secret, jwt_key, webhook_token) = tokio::try_join!(
            fetch_secret(&project_id, "STRAVA_CLIENT_SECRET"),
            fetch_secret(&project_id, "JWT_SIGNING_KEY"),
            fetch_secret(&project_id, "WEBHOOK_VERIFY_TOKEN"),
        )?;

        tracing::info!("Secrets loaded and cached");

        Ok(Self {
            strava_client_id: env::var("STRAVA_CLIENT_ID")
                .map_err(|_| ConfigError::Missing("STRAVA_CLIENT_ID"))?,
            frontend_url: env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://localhost:5173".to_string()),
            gcp_project_id: project_id,
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .unwrap_or(8080),
            strava_client_secret: client_secret.trim().to_string(),
            jwt_signing_key: jwt_key.into_bytes(),
            webhook_verify_token: webhook_token.trim().to_string(),
        })
    }
}

/// Fetch a secret from Google Secret Manager.
///
/// NOTE: This is commented out because Cloud Run injects secrets as environment
/// variables via secret bindings in Terraform. Direct Secret Manager API calls
/// are not needed - the secrets are already in env vars when the app starts.
#[cfg(feature = "gcp")]
async fn fetch_secret(_project_id: &str, secret_name: &str) -> Result<String, ConfigError> {
    // Cloud Run secret bindings inject secrets as env vars, so we just read from env
    std::env::var(secret_name)
        .map_err(|_| ConfigError::Missing(Box::leak(secret_name.to_string().into_boxed_str())))
}

/// Configuration errors
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    Missing(&'static str),

    #[error("Secret Manager error: {0}")]
    SecretManager(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_env() {
        // Set required env vars for test
        env::set_var("STRAVA_CLIENT_ID", "test_id");
        env::set_var("STRAVA_CLIENT_SECRET", "test_secret");
        env::set_var("JWT_SIGNING_KEY", "test_jwt_key_32_bytes_minimum!!");
        env::set_var("WEBHOOK_VERIFY_TOKEN", "test_verify");

        let config = Config::from_env().expect("Config should load");

        assert_eq!(config.strava_client_id, "test_id");
        assert_eq!(config.strava_client_secret, "test_secret");
        assert_eq!(config.port, 8080);
    }
}
