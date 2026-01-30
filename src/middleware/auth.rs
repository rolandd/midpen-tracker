// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! JWT authentication middleware.

use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// JWT claims structure.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    /// Subject (Strava athlete ID)
    pub sub: String,
    /// Expiration time (Unix timestamp)
    pub exp: usize,
    /// Issued at (Unix timestamp)
    pub iat: usize,
}

/// Authenticated user extracted from JWT.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub athlete_id: u64,
}

/// Middleware that requires valid JWT authentication.
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    jar: CookieJar, // Added CookieJar extractor
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Try cookie first, then header
    let token = if let Some(cookie) = jar.get("midpen_token") {
        cookie.value().to_string()
    } else {
        let auth_header = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok());

        match auth_header {
            Some(h) if h.starts_with("Bearer ") => h[7..].to_string(),
            _ => return Err(StatusCode::UNAUTHORIZED),
        }
    };

    let key = DecodingKey::from_secret(&state.config.jwt_signing_key);
    let validation = Validation::new(Algorithm::HS256);

    let token_data =
        decode::<Claims>(&token, &key, &validation).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let athlete_id: u64 = token_data
        .claims
        .sub
        .parse()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let auth_user = AuthUser { athlete_id };
    request.extensions_mut().insert(auth_user);

    Ok(next.run(request).await)
}

/// Create a JWT for a user session.
pub fn create_jwt(athlete_id: u64, signing_key: &[u8]) -> anyhow::Result<String> {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as usize;

    let claims = Claims {
        sub: athlete_id.to_string(),
        iat: now,
        exp: now + 30 * 24 * 60 * 60, // 30 days
    };

    Ok(encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(signing_key),
    )?)
}
