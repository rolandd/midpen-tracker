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
    ///
    /// Accepts optional Additional Authenticated Data (AAD) for context binding.
    pub async fn encrypt(&self, plaintext: &str, aad: Option<&[u8]>) -> Result<String, AppError> {
        use google_cloud_googleapis::cloud::kms::v1::EncryptRequest;

        // Mock mode (Debug builds only)
        #[cfg(debug_assertions)]
        {
            if self.client.is_none() {
                let data = if let Some(a) = aad {
                    format!("AAD:{}:{}", hex::encode(a), plaintext)
                } else {
                    format!("NOAAD:{}", plaintext)
                };
                return Ok(BASE64.encode(data));
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
            additional_authenticated_data: aad.unwrap_or_default().to_vec(),
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
    ///
    /// Accepts optional Additional Authenticated Data (AAD) for context verification.
    pub async fn decrypt(
        &self,
        ciphertext_b64: &str,
        aad: Option<&[u8]>,
    ) -> Result<String, AppError> {
        use google_cloud_googleapis::cloud::kms::v1::DecryptRequest;

        // Mock mode (Debug builds only)
        #[cfg(debug_assertions)]
        {
            if self.client.is_none() {
                let bytes = BASE64.decode(ciphertext_b64).map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Base64 output decode failed (mock): {}", e))
                })?;
                let decoded = String::from_utf8(bytes).map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("UTF-8 decode failed (mock): {}", e))
                })?;

                // Check for prefixes
                if let Some(rest) = decoded.strip_prefix("NOAAD:") {
                    // Encrypted without AAD
                    if aad.is_some() {
                        return Err(AppError::Internal(anyhow::anyhow!(
                            "Integrity check failed (mock): Expected AAD, found none"
                        )));
                    }
                    return Ok(rest.to_string());
                } else if let Some(rest) = decoded.strip_prefix("AAD:") {
                    // Encrypted with AAD: format "AAD:{hex_aad}:{plaintext}"
                    let parts: Vec<&str> = rest.splitn(2, ':').collect();
                    if parts.len() != 2 {
                        return Err(AppError::Internal(anyhow::anyhow!(
                            "Malformed mock ciphertext"
                        )));
                    }
                    let stored_aad_hex = parts[0];
                    let plaintext = parts[1];

                    if let Some(expected_aad) = aad {
                        if hex::encode(expected_aad) != stored_aad_hex {
                            return Err(AppError::Internal(anyhow::anyhow!(
                                "Integrity check failed (mock): AAD mismatch"
                            )));
                        }
                    } else {
                        // Expected no AAD, but found some
                        return Err(AppError::Internal(anyhow::anyhow!(
                            "Integrity check failed (mock): Found AAD, expected none"
                        )));
                    }
                    return Ok(plaintext.to_string());
                } else {
                    // Legacy legacy (raw plaintext or other format from before mock update)
                    // Treat as NOAAD if no prefix found
                    if aad.is_some() {
                        return Err(AppError::Internal(anyhow::anyhow!(
                            "Integrity check failed (mock): Expected AAD, found legacy"
                        )));
                    }
                    return Ok(decoded);
                }
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
            additional_authenticated_data: aad.unwrap_or_default().to_vec(),
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

    /// Attempt decryption with AAD, falling back to no-AAD (legacy) if it fails.
    /// This enables seamless migration for existing data.
    pub async fn decrypt_with_fallback(
        &self,
        ciphertext: &str,
        aad: Option<&[u8]>,
    ) -> Result<String, AppError> {
        // Try with AAD first
        match self.decrypt(ciphertext, aad).await {
            Ok(pt) => Ok(pt),
            Err(_) => {
                // If failed, try without AAD (Legacy fallback)
                // Note: We only fallback if we ASKED for AAD verification.
                // If we didn't ask (aad is None), then decrypt() already tried without AAD.
                if aad.is_some() {
                    tracing::warn!("Decryption with AAD failed, attempting legacy fallback");
                    self.decrypt(ciphertext, None).await
                } else {
                    Err(AppError::Internal(anyhow::anyhow!(
                        "Decryption failed (no AAD provided)"
                    )))
                }
            }
        }
    }
}

/// Helper to encrypt OAuth tokens before storing.
/// Uses athlete_id as Additional Authenticated Data (AAD) for context binding.
pub async fn encrypt_tokens(
    kms: &KmsService,
    access_token: &str,
    refresh_token: &str,
    athlete_id: u64,
) -> Result<(String, String), AppError> {
    let aad = format!("athlete_id:{}", athlete_id);
    let aad_bytes = aad.as_bytes();

    let encrypted_access = kms.encrypt(access_token, Some(aad_bytes)).await?;
    let encrypted_refresh = kms.encrypt(refresh_token, Some(aad_bytes)).await?;
    Ok((encrypted_access, encrypted_refresh))
}

/// Helper to decrypt OAuth tokens after retrieval.
/// Uses athlete_id as AAD, with fallback to legacy (no AAD) for migration.
pub async fn decrypt_tokens(
    kms: &KmsService,
    encrypted_access: &str,
    encrypted_refresh: &str,
    athlete_id: u64,
) -> Result<(String, String), AppError> {
    let aad = format!("athlete_id:{}", athlete_id);
    let aad_bytes = aad.as_bytes();

    let access_token = kms
        .decrypt_with_fallback(encrypted_access, Some(aad_bytes))
        .await?;
    let refresh_token = kms
        .decrypt_with_fallback(encrypted_refresh, Some(aad_bytes))
        .await?;
    Ok((access_token, refresh_token))
}
