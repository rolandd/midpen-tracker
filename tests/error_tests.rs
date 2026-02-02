// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

use midpen_tracker::error::AppError;

#[test]
fn test_is_strava_token_error_matches() {
    let err = AppError::StravaApi("Token expired".to_string());
    assert!(err.is_strava_token_error());

    let err = AppError::StravaApi("Invalid access token".to_string());
    assert!(err.is_strava_token_error());

    let err = AppError::StravaApi("Some other invalid thing".to_string());
    assert!(err.is_strava_token_error());

    let err = AppError::StravaApi(AppError::STRAVA_TOKEN_ERROR.to_string());
    assert!(err.is_strava_token_error());
}

#[test]
fn test_is_strava_token_error_no_match() {
    let err = AppError::StravaApi("Rate limit exceeded".to_string());
    assert!(!err.is_strava_token_error());

    let err = AppError::StravaApi("Internal Server Error".to_string());
    assert!(!err.is_strava_token_error());

    let err = AppError::BadRequest("Bad Request".to_string());
    assert!(!err.is_strava_token_error());
}
