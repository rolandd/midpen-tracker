// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Cloud KMS service for encrypting/decrypting OAuth tokens.
//!
//! Uses direct KMS encryption (not envelope encryption) for simplicity.
//! The KMS key is referenced by path, and all encrypt/decrypt calls go to KMS.

use crate::error::AppError;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use google_cloud_googleapis::cloud::kms::v1::{DecryptRequest, EncryptRequest};

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

    /// Encrypt plaintext data using KMS with optional Additional Authenticated Data (AAD).
    /// Returns base64-encoded ciphertext.
    pub async fn encrypt(&self, plaintext: &str, aad: Option<&[u8]>) -> Result<String, AppError> {
        // Mock mode (Debug builds only)
        #[cfg(debug_assertions)]
        {
            if self.client.is_none() {
                let aad_part = aad.unwrap_or(b"");
                // Simple reversible encoding: MOCK_V1:B64(AAD):B64(PT)
                return Ok(format!(
                    "MOCK_V1:{}:{}",
                    BASE64.encode(aad_part),
                    BASE64.encode(plaintext)
                ));
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
            additional_authenticated_data: aad.unwrap_or(b"").to_vec(),
            ..Default::default()
        };

        let response = client
            .encrypt(req, None)
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("KMS encrypt failed: {}", e)))?;

        let ciphertext = response.ciphertext; // Vec<u8>
        Ok(BASE64.encode(ciphertext))
    }

    /// Decrypt ciphertext using KMS with optional Additional Authenticated Data (AAD).
    /// Expects base64-encoded ciphertext.
    pub async fn decrypt(
        &self,
        ciphertext_b64: &str,
        aad: Option<&[u8]>,
    ) -> Result<String, AppError> {
        // Mock Implementation
        // Mock mode (Debug builds only)
        #[cfg(debug_assertions)]
        {
            if self.client.is_none() {
                if let Some(rest) = ciphertext_b64.strip_prefix("MOCK_V1:") {
                    let parts: Vec<&str> = rest.split(':').collect();
                    if parts.len() != 2 {
                        return Err(AppError::Internal(anyhow::anyhow!(
                            "Invalid mock ciphertext format"
                        )));
                    }
                    let stored_aad = BASE64.decode(parts[0]).map_err(|e| {
                        AppError::Internal(anyhow::anyhow!("Invalid mock AAD encoding: {}", e))
                    })?;
                    let plaintext = BASE64.decode(parts[1]).map_err(|e| {
                        AppError::Internal(anyhow::anyhow!(
                            "Invalid mock plaintext encoding: {}",
                            e
                        ))
                    })?;

                    // Verify AAD
                    let provided_aad = aad.unwrap_or(b"");
                    if stored_aad != provided_aad {
                        return Err(AppError::Internal(anyhow::anyhow!("Mock KMS AAD mismatch")));
                    }

                    return String::from_utf8(plaintext).map_err(|e| {
                        AppError::Internal(anyhow::anyhow!("Invalid mock plaintext UTF-8: {}", e))
                    });
                } else {
                    return Err(AppError::Internal(anyhow::anyhow!(
                        "Unknown mock ciphertext version"
                    )));
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
            additional_authenticated_data: aad.unwrap_or(b"").to_vec(),
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

    /// Decrypt with fallback for legacy tokens (no AAD).
    /// Tries decrypting with provided AAD first. If that fails, tries with empty AAD.
    /// This allows migrating from non-AAD encrypted tokens to AAD-encrypted ones seamlessly.
    pub async fn decrypt_or_fallback(
        &self,
        ciphertext_b64: &str,
        aad: &[u8],
    ) -> Result<String, AppError> {
        // Try with AAD
        match self.decrypt(ciphertext_b64, Some(aad)).await {
            Ok(plaintext) => Ok(plaintext),
            Err(e) => {
                // Failed - try fallback (no AAD)
                tracing::warn!(
                    error = %e,
                    "Decryption with AAD failed, attempting legacy fallback (no AAD)"
                );
                self.decrypt(ciphertext_b64, None).await
            }
        }
    }
}

/// Helper to encrypt OAuth tokens before storing.
pub async fn encrypt_tokens(
    kms: &KmsService,
    access_token: &str,
    refresh_token: &str,
    athlete_id: u64,
) -> Result<(String, String), AppError> {
    let aad = athlete_id.to_be_bytes();

    let encrypted_access = kms.encrypt(access_token, Some(&aad)).await?;
    let encrypted_refresh = kms.encrypt(refresh_token, Some(&aad)).await?;
    Ok((encrypted_access, encrypted_refresh))
}

/// Helper to decrypt OAuth tokens after retrieval.
pub async fn decrypt_tokens(
    kms: &KmsService,
    encrypted_access: &str,
    encrypted_refresh: &str,
    athlete_id: u64,
) -> Result<(String, String), AppError> {
    let aad = athlete_id.to_be_bytes();

    let access_token = kms.decrypt_or_fallback(encrypted_access, &aad).await?;
    let refresh_token = kms.decrypt_or_fallback(encrypted_refresh, &aad).await?;
    Ok((access_token, refresh_token))
}
