// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Google OIDC token verification for Cloud Tasks callbacks.

use crate::config::Config;
use anyhow::Context;
use axum::http::HeaderValue;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::header::CACHE_CONTROL;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock};

const DISCOVERY_URL: &str = "https://accounts.google.com/.well-known/openid-configuration";
const DEFAULT_JWKS_URL: &str = "https://www.googleapis.com/oauth2/v3/certs";
const DEFAULT_HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_CACHE_TTL: Duration = Duration::from_secs(300);
const CLOCK_SKEW_SECS: u64 = 60;

/// Verified Cloud Tasks principal extracted from a valid OIDC token.
#[derive(Debug, Clone)]
pub struct VerifiedTaskPrincipal {
    pub email: String,
    pub subject: String,
    pub audience: String,
}

/// OIDC verification error categories.
#[derive(Debug, Clone)]
pub enum OidcError {
    /// The token is missing/invalid or claims do not match expectations.
    Forbidden(String),
    /// A transient infrastructure failure occurred (safe for task retry).
    Transient(String),
}

#[derive(Clone)]
enum VerifierMode {
    Google,
    StaticKey {
        kid: String,
        decoding_key: Arc<DecodingKey>,
    },
}

#[derive(Clone)]
struct DiscoveryCacheEntry {
    jwks_uri: String,
    expires_at: Instant,
}

#[derive(Clone)]
struct JwksCacheEntry {
    keys_by_kid: HashMap<String, Arc<DecodingKey>>,
    expires_at: Instant,
}

/// Verifier for Cloud Tasks-issued OIDC ID tokens.
pub struct GoogleOidcVerifier {
    http_client: reqwest::Client,
    expected_audience: String,
    expected_service_account_email: String,
    mode: VerifierMode,
    discovery_cache: RwLock<Option<DiscoveryCacheEntry>>,
    jwks_cache: RwLock<Option<JwksCacheEntry>>,
    refresh_lock: Mutex<()>,
}

impl GoogleOidcVerifier {
    /// Create a production verifier that discovers and caches Google JWKS keys.
    pub fn new(config: &Config) -> anyhow::Result<Self> {
        let http_client = reqwest::Client::builder()
            .timeout(DEFAULT_HTTP_TIMEOUT)
            .build()
            .context("failed building OIDC HTTP client")?;

        let expected_audience = canonicalize_audience(&config.api_url);
        let expected_service_account_email = format!(
            "midpen-tracker-api@{}.iam.gserviceaccount.com",
            config.gcp_project_id
        );

        tracing::info!(
            expected_audience = %expected_audience,
            expected_service_account_email = %expected_service_account_email,
            "Initialized Cloud Tasks OIDC verifier"
        );

        Ok(Self {
            http_client,
            expected_audience,
            expected_service_account_email,
            mode: VerifierMode::Google,
            discovery_cache: RwLock::new(None),
            jwks_cache: RwLock::new(None),
            refresh_lock: Mutex::new(()),
        })
    }

    /// Create a verifier with a static RSA public key.
    ///
    /// This is intended for deterministic local/integration tests.
    pub fn new_with_static_key(
        config: &Config,
        kid: impl Into<String>,
        decoding_key: DecodingKey,
    ) -> anyhow::Result<Self> {
        let kid = kid.into();
        if kid.trim().is_empty() {
            anyhow::bail!("static OIDC kid must not be empty");
        }

        let http_client = reqwest::Client::builder()
            .timeout(DEFAULT_HTTP_TIMEOUT)
            .build()
            .context("failed building OIDC HTTP client")?;

        let expected_audience = canonicalize_audience(&config.api_url);
        let expected_service_account_email = format!(
            "midpen-tracker-api@{}.iam.gserviceaccount.com",
            config.gcp_project_id
        );

        Ok(Self {
            http_client,
            expected_audience,
            expected_service_account_email,
            mode: VerifierMode::StaticKey {
                kid,
                decoding_key: Arc::new(decoding_key),
            },
            discovery_cache: RwLock::new(None),
            jwks_cache: RwLock::new(None),
            refresh_lock: Mutex::new(()),
        })
    }

    /// Verify a Cloud Tasks OIDC bearer token from an Authorization header.
    pub async fn verify_cloud_tasks_token(
        &self,
        auth_header: Option<&HeaderValue>,
    ) -> Result<VerifiedTaskPrincipal, OidcError> {
        let token = extract_bearer_token(auth_header)?;

        let header = decode_header(token)
            .map_err(|e| OidcError::Forbidden(format!("invalid JWT header: {e}")))?;

        if header.alg != Algorithm::RS256 {
            return Err(OidcError::Forbidden(format!(
                "unexpected JWT alg: {:?}",
                header.alg
            )));
        }

        let kid = header
            .kid
            .ok_or_else(|| OidcError::Forbidden("missing JWT kid".to_string()))?;

        let decoding_key = self.decoding_key_for_kid(&kid).await?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
        validation.set_audience(&[self.expected_audience.as_str()]);
        validation.validate_nbf = true;
        validation.leeway = CLOCK_SKEW_SECS;

        let token_data =
            decode::<GoogleIdTokenClaims>(token, decoding_key.as_ref(), &validation)
                .map_err(|e| OidcError::Forbidden(format!("JWT validation failed: {e}")))?;

        let claims = token_data.claims;

        tracing::info!(
            email = claims.email.as_deref().unwrap_or("<missing>"),
            email_verified = ?claims.email_verified,
            subject = %claims.sub,
            audience = %claims.aud,
            issuer = %claims.iss,
            exp = claims.exp,
            nbf = ?claims.nbf,
            "Cloud Tasks OIDC claims"
        );

        validate_iat(claims.iat)?;

        let email = claims
            .email
            .ok_or_else(|| OidcError::Forbidden("missing email claim".to_string()))?;

        if email != self.expected_service_account_email {
            return Err(OidcError::Forbidden(format!(
                "unexpected service account email: {email}"
            )));
        }

        match claims.email_verified {
            Some(true) => {}
            Some(false) => {
                return Err(OidcError::Forbidden(
                    "email_verified claim is false".to_string(),
                ));
            }
            None => {
                return Err(OidcError::Forbidden(
                    "email_verified claim is missing".to_string(),
                ));
            }
        }

        Ok(VerifiedTaskPrincipal {
            email,
            subject: claims.sub,
            audience: claims.aud,
        })
    }

    async fn decoding_key_for_kid(&self, kid: &str) -> Result<Arc<DecodingKey>, OidcError> {
        match &self.mode {
            VerifierMode::StaticKey {
                kid: static_kid,
                decoding_key,
            } => {
                if kid == static_kid {
                    return Ok(decoding_key.clone());
                }

                return Err(OidcError::Forbidden(format!(
                    "unknown JWT kid for static verifier: {kid}"
                )));
            }
            VerifierMode::Google => {}
        }

        if let Some(key) = self.lookup_cached_key(kid).await {
            return Ok(key);
        }

        for force_refresh in [false, true] {
            self.refresh_jwks(force_refresh).await?;
            if let Some(key) = self.lookup_cached_key(kid).await {
                return Ok(key);
            }
        }

        Err(OidcError::Forbidden(format!(
            "JWT kid not found in JWKS after refresh: {kid}"
        )))
    }

    async fn lookup_cached_key(&self, kid: &str) -> Option<Arc<DecodingKey>> {
        let cache = self.jwks_cache.read().await;
        let now = Instant::now();
        cache
            .as_ref()
            .filter(|entry| entry.expires_at > now)
            .and_then(|entry| entry.keys_by_kid.get(kid))
            .cloned()
    }

    async fn refresh_jwks(&self, force_refresh: bool) -> Result<(), OidcError> {
        let _guard = self.refresh_lock.lock().await;

        if !force_refresh {
            let cache = self.jwks_cache.read().await;
            if cache
                .as_ref()
                .is_some_and(|entry| entry.expires_at > Instant::now())
            {
                return Ok(());
            }
        }

        let jwks_uri = self.resolve_jwks_uri(force_refresh).await;
        let jwks_uri = match jwks_uri {
            Ok(uri) => uri,
            Err(e) => {
                tracing::error!(error = ?e, "Failed to resolve JWKS URI");
                return Err(e);
            }
        };

        tracing::debug!(jwks_uri = %jwks_uri, "Refreshing Google JWKS cache");

        let response = self
            .http_client
            .get(&jwks_uri)
            .send()
            .await
            .map_err(|e| OidcError::Transient(format!("JWKS request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(OidcError::Transient(format!(
                "JWKS request returned status {}",
                response.status()
            )));
        }

        let ttl = cache_ttl_from_headers(response.headers(), DEFAULT_CACHE_TTL);

        let jwks: Jwks = response
            .json()
            .await
            .map_err(|e| OidcError::Transient(format!("invalid JWKS JSON: {e}")))?;

        let mut keys_by_kid: HashMap<String, Arc<DecodingKey>> = HashMap::new();

        for jwk in jwks.keys {
            if jwk.kty != "RSA" {
                continue;
            }

            if jwk.kid.trim().is_empty() {
                continue;
            }

            if let Some(alg) = &jwk.alg {
                if alg != "RS256" {
                    continue;
                }
            }

            if let Some(use_) = &jwk.use_ {
                if use_ != "sig" {
                    continue;
                }
            }

            match DecodingKey::from_rsa_components(&jwk.n, &jwk.e) {
                Ok(key) => {
                    keys_by_kid.insert(jwk.kid, Arc::new(key));
                }
                Err(e) => {
                    tracing::warn!(error = %e, kid = %jwk.kid, "Skipping invalid RSA JWKS key");
                }
            }
        }

        if keys_by_kid.is_empty() {
            return Err(OidcError::Transient(
                "JWKS response did not include any usable RSA keys".to_string(),
            ));
        }

        let entry = JwksCacheEntry {
            keys_by_kid,
            expires_at: Instant::now() + ttl,
        };

        *self.jwks_cache.write().await = Some(entry);

        tracing::debug!(ttl_secs = ttl.as_secs(), "Google JWKS cache refreshed");
        Ok(())
    }

    async fn resolve_jwks_uri(&self, force_refresh: bool) -> Result<String, OidcError> {
        if !force_refresh {
            let cache = self.discovery_cache.read().await;
            if let Some(entry) = cache
                .as_ref()
                .filter(|entry| entry.expires_at > Instant::now())
            {
                return Ok(entry.jwks_uri.clone());
            }
        }

        let cached_jwks_uri = self
            .discovery_cache
            .read()
            .await
            .as_ref()
            .map(|entry| entry.jwks_uri.clone());

        let response = self.http_client.get(DISCOVERY_URL).send().await;
        match response {
            Ok(resp) if resp.status().is_success() => {
                let ttl = cache_ttl_from_headers(resp.headers(), DEFAULT_CACHE_TTL);
                let discovery: OpenIdConfig = resp
                    .json()
                    .await
                    .map_err(|e| OidcError::Transient(format!("invalid discovery JSON: {e}")))?;

                *self.discovery_cache.write().await = Some(DiscoveryCacheEntry {
                    jwks_uri: discovery.jwks_uri.clone(),
                    expires_at: Instant::now() + ttl,
                });

                Ok(discovery.jwks_uri)
            }
            Ok(resp) => {
                tracing::warn!(
                    status = %resp.status(),
                    "OIDC discovery returned non-success status; using fallback JWKS URI"
                );
                Ok(cached_jwks_uri.unwrap_or_else(|| DEFAULT_JWKS_URL.to_string()))
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "OIDC discovery request failed; using fallback JWKS URI"
                );
                Ok(cached_jwks_uri.unwrap_or_else(|| DEFAULT_JWKS_URL.to_string()))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenIdConfig {
    jwks_uri: String,
}

#[derive(Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    alg: Option<String>,
    n: String,
    e: String,
    #[serde(rename = "use")]
    use_: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleIdTokenClaims {
    iss: String,
    aud: String,
    sub: String,
    exp: usize,
    iat: Option<usize>,
    nbf: Option<usize>,
    email: Option<String>,
    email_verified: Option<bool>,
}

fn extract_bearer_token(auth_header: Option<&HeaderValue>) -> Result<&str, OidcError> {
    let value = auth_header
        .ok_or_else(|| OidcError::Forbidden("missing Authorization header".to_string()))?
        .to_str()
        .map_err(|_| OidcError::Forbidden("invalid Authorization header".to_string()))?;

    let token = value.strip_prefix("Bearer ").ok_or_else(|| {
        OidcError::Forbidden("Authorization header must be Bearer token".to_string())
    })?;

    if token.is_empty() {
        return Err(OidcError::Forbidden("Bearer token is empty".to_string()));
    }

    Ok(token)
}

fn validate_iat(iat: Option<usize>) -> Result<(), OidcError> {
    let now = now_unix_secs();

    let Some(iat) = iat else {
        return Err(OidcError::Forbidden("missing iat claim".to_string()));
    };

    if iat as u64 > now + CLOCK_SKEW_SECS {
        return Err(OidcError::Forbidden(
            "iat claim is in the future".to_string(),
        ));
    }

    Ok(())
}

fn cache_ttl_from_headers(headers: &reqwest::header::HeaderMap, fallback: Duration) -> Duration {
    let Some(cache_control) = headers
        .get(CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .and_then(parse_cache_control_max_age)
    else {
        return fallback;
    };

    Duration::from_secs(cache_control)
}

fn parse_cache_control_max_age(value: &str) -> Option<u64> {
    for directive in value.split(',') {
        let directive = directive.trim();

        if let Some(raw) = directive.strip_prefix("max-age=") {
            let raw = raw.trim_matches('"');
            if let Ok(seconds) = raw.parse::<u64>() {
                return Some(seconds);
            }
        }
    }

    None
}

fn canonicalize_audience(audience: &str) -> String {
    audience.trim_end_matches('/').to_string()
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cache_control_max_age_valid() {
        assert_eq!(
            parse_cache_control_max_age("public, max-age=3600"),
            Some(3600)
        );
        assert_eq!(parse_cache_control_max_age("max-age=60"), Some(60));
        assert_eq!(parse_cache_control_max_age("max-age=\"120\""), Some(120));
    }

    #[test]
    fn parse_cache_control_max_age_invalid() {
        assert_eq!(parse_cache_control_max_age("public, immutable"), None);
        assert_eq!(parse_cache_control_max_age("max-age=abc"), None);
        assert_eq!(parse_cache_control_max_age(""), None);
    }

    #[test]
    fn extract_bearer_token_errors() {
        assert!(matches!(
            extract_bearer_token(None),
            Err(OidcError::Forbidden(_))
        ));

        let bad = HeaderValue::from_static("Basic abc");
        assert!(matches!(
            extract_bearer_token(Some(&bad)),
            Err(OidcError::Forbidden(_))
        ));

        let empty = HeaderValue::from_static("Bearer ");
        assert!(matches!(
            extract_bearer_token(Some(&empty)),
            Err(OidcError::Forbidden(_))
        ));
    }
}
