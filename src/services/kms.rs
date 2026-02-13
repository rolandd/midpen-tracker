// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Cloud KMS service for encrypting/decrypting OAuth tokens.
//!
//! Uses direct KMS encryption (not envelope encryption) for simplicity.
//! The KMS key is referenced by path, and all encrypt/decrypt calls go to KMS.

use crate::error::AppError;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

/// KMS encryption service.
#[derive(Clone)]
pub struct KmsService {
    /// Full resource path to the KMS key
    /// Format: projects/{project}/locations/{location}/keyRings/{ring}/cryptoKeys/{key}
    key_path: String,

    /// GCP KMS client
    client: Option<std::sync::Arc<google_cloud_kms::client::Client>>,
}

impl KmsService {
    /// KMS Key Ring Name
    const KEY_RING_NAME: &str = "midpen-tracker";

    /// Create a new KMS service.
    /// Connects to GCP KMS.
    pub async fn new(project_id: &str, location: &str, key_name: &str) -> Result<Self, AppError> {
        let key_path = format!(
            "projects/{}/locations/{}/keyRings/{}/cryptoKeys/{}",
            project_id,
            location,
            Self::KEY_RING_NAME,
            key_name
        );

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

    /// Create a mock KMS service for testing (offline mode).
    /// Only available in debug/test builds.
    #[cfg(debug_assertions)]
    pub fn new_mock() -> Self {
        Self {
            key_path: "projects/mock/locations/mock/keyRings/mock/cryptoKeys/mock".to_string(),
            client: None,
        }
    }

    /// Encrypt plaintext data using KMS.
    /// Returns base64-encoded ciphertext.
    pub async fn encrypt(&self, plaintext: &str) -> Result<String, AppError> {
        use google_cloud_googleapis::cloud::kms::v1::EncryptRequest;

        // Mock mode (Debug builds only)
        #[cfg(debug_assertions)]
        {
            if self.client.is_none() {
                return Ok(BASE64.encode(plaintext));
            }
        }

        // Production/Real mode
        // In release builds, this check ensures we return an error if the
        // client is missing, preventing insecure operations.
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

    /// Decrypt ciphertext using KMS.
    /// Expects base64-encoded ciphertext.
    pub async fn decrypt(&self, ciphertext_b64: &str) -> Result<String, AppError> {
        use google_cloud_googleapis::cloud::kms::v1::DecryptRequest;

        // Mock mode (Debug builds only)
        #[cfg(debug_assertions)]
        {
            if self.client.is_none() {
                let bytes = BASE64.decode(ciphertext_b64).map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Base64 output decode failed (mock): {}", e))
                })?;
                return String::from_utf8(bytes).map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("UTF-8 decode failed (mock): {}", e))
                });
            }
        }

        // Production/Real mode
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
