//! JWT authentication middleware.

use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
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
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    let key = DecodingKey::from_secret(&state.config.jwt_signing_key);
    let validation = Validation::new(Algorithm::HS256);

    let token_data =
        decode::<Claims>(token, &key, &validation).map_err(|_| StatusCode::UNAUTHORIZED)?;

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
pub fn create_jwt(
    athlete_id: u64,
    signing_key: &[u8],
) -> Result<String, jsonwebtoken::errors::Error> {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = Claims {
        sub: athlete_id.to_string(),
        iat: now,
        exp: now + 30 * 24 * 60 * 60, // 30 days
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(signing_key),
    )
}
