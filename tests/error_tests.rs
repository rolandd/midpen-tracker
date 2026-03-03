// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

use midpen_tracker::error::{AppError, StravaError};

#[test]
fn test_is_strava_token_error_matches() {
    let err = AppError::StravaApi(StravaError::TokenInvalid);
    assert!(err.is_strava_token_error());
}

#[test]
fn test_is_strava_token_error_no_match() {
    let err = AppError::StravaApi(StravaError::RateLimit);
    assert!(!err.is_strava_token_error());

    let err = AppError::StravaApi(StravaError::Other("Internal Server Error".to_string()));
    assert!(!err.is_strava_token_error());

    let err = AppError::BadRequest("Bad Request".to_string());
    assert!(!err.is_strava_token_error());
}

#[test]
fn test_is_db_aborted_normalization() {
    let fs_err = firestore::errors::FirestoreError::DatabaseError(
        firestore::errors::FirestoreDatabaseError {
            public: firestore::errors::FirestoreErrorPublicGenericDetails {
                code: "Aborted".to_string(),
            },
            details: "test".to_string(),
            retry_possible: false,
        },
    );
    // Should be normalized to DbError::Aborted
    let err = AppError::from(fs_err);
    assert!(err.is_db_aborted());
}

#[test]
fn test_is_db_aborted_no_match() {
    let fs_err = firestore::errors::FirestoreError::DatabaseError(
        firestore::errors::FirestoreDatabaseError {
            public: firestore::errors::FirestoreErrorPublicGenericDetails {
                code: "NotFound".to_string(),
            },
            details: "test".to_string(),
            retry_possible: false,
        },
    );
    let err = AppError::from(fs_err);
    assert!(!err.is_db_aborted());
}
