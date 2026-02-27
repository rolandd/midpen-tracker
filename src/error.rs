// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Application error types with consistent API responses.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Application error type that converts to HTTP responses.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Authentication required")]
    Unauthorized,

    #[error("Invalid or expired token")]
    InvalidToken,

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Strava API error: {0}")]
    StravaApi(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Firestore error: {0}")]
    Firestore(#[from] firestore::errors::FirestoreError),

    #[error("Internal server error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub const STRAVA_TOKEN_ERROR: &'static str = "Token expired or invalid";
    pub const STRAVA_RATE_LIMIT: &'static str = "Rate limit exceeded";

    /// Check if this error indicates a Strava token issue (expired/revoked).
    pub fn is_strava_token_error(&self) -> bool {
        match self {
            AppError::StravaApi(msg) => {
                msg.contains("Token expired") || msg.contains("invalid") || msg.contains("Invalid")
            }
            _ => false,
        }
    }

    /// Check if this error indicates a database transaction conflict (ABORTED).
    pub fn is_db_aborted(&self) -> bool {
        match self {
            AppError::Firestore(firestore::errors::FirestoreError::DatabaseError(ref e)) => {
                e.public.code == "Aborted"
            }
            AppError::Database(msg) => {
                // Check for common Firestore/gRPC aborted error strings
                msg.contains("Aborted") || msg.contains("contention") || msg.contains("ABORTED")
            }
            _ => false,
        }
    }
}

/// JSON error response body
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, details) = match &self {
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", None),
            AppError::InvalidToken => (StatusCode::UNAUTHORIZED, "invalid_token", None),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", Some(msg.clone())),
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, "bad_request", Some(msg.clone()))
            }
            AppError::StravaApi(msg) => {
                (StatusCode::BAD_GATEWAY, "strava_error", Some(msg.clone()))
            }
            AppError::Database(msg) => {
                tracing::error!(error = %msg, "Database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "database_error", None)
            }
            AppError::Firestore(err) => {
                tracing::error!(error = ?err, "Firestore error");
                (StatusCode::INTERNAL_SERVER_ERROR, "database_error", None)
            }
            AppError::Internal(err) => {
                tracing::error!(error = %err, "Internal server error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", None)
            }
        };

        let body = ErrorResponse {
            error: error.to_string(),
            details,
        };

        (status, Json(body)).into_response()
    }
}

/// Result type alias for handlers
pub type Result<T> = std::result::Result<T, AppError>;
