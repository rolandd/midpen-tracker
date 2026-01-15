//! Cloud KMS service for encrypting/decrypting OAuth tokens.
//!
//! Uses direct KMS encryption (not envelope encryption) for simplicity.
//! The KMS key is referenced by path, and all encrypt/decrypt calls go to KMS.

use crate::error::AppError;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

/// KMS encryption service.
///
/// When the `gcp` feature is enabled, calls Google Cloud KMS.
/// Without the feature, provides stub implementations for local dev.
#[derive(Clone)]
pub struct KmsService {
    /// Full resource path to the KMS key
    /// Format: projects/{project}/locations/{location}/keyRings/{ring}/cryptoKeys/{key}
    #[allow(dead_code)]
    key_path: String,

    /// GCP KMS client (only present when feature enabled)
    #[cfg(feature = "gcp")]
    client: Option<std::sync::Arc<google_cloud_kms::client::Client>>,
}

impl KmsService {
    /// Create a new KMS service.
    /// Connects to GCP KMS if the `gcp` feature is enabled.
    pub async fn new(
        project_id: &str,
        location: &str,
        key_ring: &str,
        key_name: &str,
    ) -> Result<Self, AppError> {
        let key_path = format!(
            "projects/{}/locations/{}/keyRings/{}/cryptoKeys/{}",
            project_id, location, key_ring, key_name
        );

        #[cfg(feature = "gcp")]
        {
            let config = google_cloud_kms::client::ClientConfig::default()
                .with_auth()
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Failed to create KMS auth config: {}", e))
                })?;

            let client = google_cloud_kms::client::Client::new(config)
                .await
                .map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Failed to create KMS client: {}", e))
                })?;

            Ok(Self {
                key_path,
                client: Some(std::sync::Arc::new(client)),
            })
        }

        #[cfg(not(feature = "gcp"))]
        {
            Ok(Self { key_path })
        }
    }

    /// Encrypt plaintext data using KMS.
    /// Returns base64-encoded ciphertext.
    #[cfg(feature = "gcp")]
    pub async fn encrypt(&self, plaintext: &str) -> Result<String, AppError> {
        use google_cloud_googleapis::cloud::kms::v1::EncryptRequest;

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("KMS client not connected")))?;

        let req = EncryptRequest {
            name: self.key_path.clone(),
            plaintext: plaintext.as_bytes().to_vec(),
            ..Default::default()
        };

        let response = client
            .encrypt(req, None)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("KMS encrypt failed: {}", e)))?;

        let ciphertext = response.ciphertext; // Vec<u8>
        Ok(BASE64.encode(ciphertext))
    }

    /// Encrypt plaintext data (stub for local dev).
    #[cfg(not(feature = "gcp"))]
    pub async fn encrypt(&self, plaintext: &str) -> Result<String, AppError> {
        tracing::warn!("Using stub KMS encryption - NOT SECURE FOR PRODUCTION");
        // For local dev, just base64 encode (NOT SECURE - development only)
        Ok(BASE64.encode(plaintext.as_bytes()))
    }

    /// Decrypt ciphertext using KMS.
    /// Expects base64-encoded ciphertext.
    #[cfg(feature = "gcp")]
    pub async fn decrypt(&self, ciphertext_b64: &str) -> Result<String, AppError> {
        use google_cloud_googleapis::cloud::kms::v1::DecryptRequest;

        let client = self
            .client
            .as_ref()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("KMS client not connected")))?;

        let ciphertext = BASE64.decode(ciphertext_b64).map_err(|e| {
            AppError::Internal(anyhow::anyhow!("Base64 output decode failed: {}", e))
        })?;

        let req = DecryptRequest {
            name: self.key_path.clone(),
            ciphertext,
            ..Default::default()
        };

        let response = client
            .decrypt(req, None)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("KMS decrypt failed: {}", e)))?;

        // response.plaintext is Vec<u8>
        String::from_utf8(response.plaintext)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("UTF-8 decode failed: {}", e)))
    }

    /// Decrypt ciphertext (stub for local dev).
    #[cfg(not(feature = "gcp"))]
    pub async fn decrypt(&self, ciphertext_b64: &str) -> Result<String, AppError> {
        tracing::warn!("Using stub KMS decryption - NOT SECURE FOR PRODUCTION");
        // For local dev, just base64 decode
        let bytes = BASE64
            .decode(ciphertext_b64)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Base64 decode failed: {}", e)))?;
        String::from_utf8(bytes)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("UTF-8 decode failed: {}", e)))
    }
}

/// Helper to encrypt OAuth tokens before storing.
pub async fn encrypt_tokens(
    kms: &KmsService,
    access_token: &str,
    refresh_token: &str,
) -> Result<(String, String), AppError> {
    let encrypted_access = kms.encrypt(access_token).await?;
    let encrypted_refresh = kms.encrypt(refresh_token).await?;
    Ok((encrypted_access, encrypted_refresh))
}

/// Helper to decrypt OAuth tokens after retrieval.
pub async fn decrypt_tokens(
    kms: &KmsService,
    encrypted_access: &str,
    encrypted_refresh: &str,
) -> Result<(String, String), AppError> {
    let access_token = kms.decrypt(encrypted_access).await?;
    let refresh_token = kms.decrypt(encrypted_refresh).await?;
    Ok((access_token, refresh_token))
}
