// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Application error types with consistent API responses.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ResourceNotFound {
    #[error("User {0} not found")]
    User(u64),
    #[error("Tokens for athlete {0} not found")]
    Tokens(u64),
    #[error("Activity {0} not found")]
    Activity(u64),
    #[error("{0}")]
    Other(String),
}

#[derive(Debug, thiserror::Error)]
pub enum StravaError {
    #[error("Rate limit exceeded")]
    RateLimit,
    #[error("Token expired or invalid")]
    TokenInvalid,
    #[error("Invalid grant (refresh token race)")]
    InvalidGrant,
    #[error("Resource not found (404)")]
    NotFound,
    #[error("Network error: {0}")]
    Network(String),
    #[error("API returned {0}: {1}")]
    ApiError(reqwest::StatusCode, String),
    #[error("Failed to parse response: {0}")]
    Parse(String),
    #[error("{0}")]
    Other(String),
}

mod private {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct InternalSeal;
}
use private::InternalSeal;

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("Transaction aborted due to contention")]
    Aborted,
    #[error("Firestore error: {0}")]
    Firestore(firestore::errors::FirestoreError, InternalSeal),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("{0}")]
    Other(String),
}

impl From<firestore::errors::FirestoreError> for DbError {
    fn from(err: firestore::errors::FirestoreError) -> Self {
        let is_aborted = match &err {
            firestore::errors::FirestoreError::DatabaseError(e) => e.public.code == "Aborted",
            firestore::errors::FirestoreError::DataConflictError(e) => e.public.code == "Aborted",
            _ => false,
        };

        if is_aborted {
            DbError::Aborted
        } else {
            DbError::Firestore(err, InternalSeal)
        }
    }
}

/// Application error type that converts to HTTP responses.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Authentication required")]
    Unauthorized,

    #[error("Invalid or expired token")]
    InvalidToken,

    #[error("Resource not found: {0}")]
    NotFound(#[from] ResourceNotFound),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Strava API error: {0}")]
    StravaApi(#[from] StravaError),

    #[error("Database error: {0}")]
    Database(#[from] DbError),

    #[error("Validation error: {0}")]
    Validation(#[from] validator::ValidationErrors),

    #[error("Internal server error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl From<firestore::errors::FirestoreError> for AppError {
    fn from(err: firestore::errors::FirestoreError) -> Self {
        AppError::Database(DbError::from(err))
    }
}

impl AppError {
    pub const STRAVA_TOKEN_ERROR: &'static str = "Token expired or invalid";
    pub const STRAVA_RATE_LIMIT: &'static str = "Rate limit exceeded";

    /// Check if this error indicates a Strava token issue (expired/revoked).
    pub fn is_strava_token_error(&self) -> bool {
        matches!(self, AppError::StravaApi(StravaError::TokenInvalid))
    }

    /// Check if this error indicates a database transaction conflict (ABORTED).
    pub fn is_db_aborted(&self) -> bool {
        matches!(self, AppError::Database(DbError::Aborted))
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
            AppError::NotFound(err) => (StatusCode::NOT_FOUND, "not_found", Some(err.to_string())),
            AppError::BadRequest(msg) => {
                (StatusCode::BAD_REQUEST, "bad_request", Some(msg.clone()))
            }
            AppError::Validation(errs) => {
                // Return a clean string for the simple ErrorResponse
                // The actual field-level errors are in the 'errs' struct
                let msg = format!("Validation failed: {}", errs);
                (StatusCode::BAD_REQUEST, "validation_error", Some(msg))
            }
            AppError::StravaApi(err) => (
                StatusCode::BAD_GATEWAY,
                "strava_error",
                Some(err.to_string()),
            ),
            AppError::Database(err) => {
                tracing::error!(error = ?err, "Database error");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_strava_token_error() {
        assert!(AppError::StravaApi(StravaError::TokenInvalid).is_strava_token_error());
        assert!(!AppError::StravaApi(StravaError::RateLimit).is_strava_token_error());
        assert!(!AppError::Unauthorized.is_strava_token_error());
    }

    #[test]
    fn test_is_db_aborted() {
        assert!(AppError::Database(DbError::Aborted).is_db_aborted());
        assert!(!AppError::Database(DbError::Connection("foo".to_string())).is_db_aborted());

        let fs_err = firestore::errors::FirestoreError::DatabaseError(
            firestore::errors::FirestoreDatabaseError {
                public: firestore::errors::FirestoreErrorPublicGenericDetails {
                    code: "Aborted".to_string(),
                },
                details: "test".to_string(),
                retry_possible: false,
            },
        );
        assert!(AppError::from(fs_err).is_db_aborted());
    }
}
