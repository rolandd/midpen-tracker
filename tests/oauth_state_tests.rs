// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@kernel.org>

//! OAuth state encoding/decoding tests.
//!
//! These tests verify that frontend URLs survive the encode/decode
//! roundtrip through the OAuth state parameter.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

/// Encode frontend URL into OAuth state (mirrors auth.rs logic).
fn encode_state(frontend_url: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let state_data = format!("{}|{:x}", frontend_url, timestamp);
    URL_SAFE_NO_PAD.encode(state_data.as_bytes())
}

/// Decode frontend URL from OAuth state (mirrors auth.rs logic).
fn decode_state_frontend(state: &str) -> Option<String> {
    let bytes = URL_SAFE_NO_PAD.decode(state).ok()?;
    let state_str = String::from_utf8(bytes).ok()?;
    let parts: Vec<&str> = state_str.splitn(2, '|').collect();
    parts.first().map(|s| s.to_string())
}

#[test]
fn test_oauth_state_roundtrip_localhost() {
    let frontend_url = "http://localhost:5173";
    let state = encode_state(frontend_url);
    let decoded = decode_state_frontend(&state);

    assert_eq!(decoded, Some(frontend_url.to_string()));
}

#[test]
fn test_oauth_state_roundtrip_production() {
    let frontend_url = "https://midpen-strava.pages.dev";
    let state = encode_state(frontend_url);
    let decoded = decode_state_frontend(&state);

    assert_eq!(decoded, Some(frontend_url.to_string()));
}

#[test]
fn test_oauth_state_with_path() {
    // Frontend URLs shouldn't have paths, but verify robustness
    let frontend_url = "https://example.com/some/path";
    let state = encode_state(frontend_url);
    let decoded = decode_state_frontend(&state);

    assert_eq!(decoded, Some(frontend_url.to_string()));
}

#[test]
fn test_oauth_state_decode_invalid() {
    // Invalid base64 should return None
    assert_eq!(decode_state_frontend("not-valid-base64!!!"), None);

    // Empty string decodes to empty URL (which is still "valid" base64)
    // This is acceptable - the URL validation happens at a higher level
    let empty_result = decode_state_frontend("");
    assert!(empty_result.is_none() || empty_result == Some("".to_string()));
}

#[test]
fn test_oauth_state_base64_url_safe() {
    // Verify we're using URL-safe base64 (no + or /)
    let frontend_url = "https://example.com";
    let state = encode_state(frontend_url);

    assert!(!state.contains('+'), "State should not contain '+'");
    assert!(!state.contains('/'), "State should not contain '/'");
    assert!(!state.contains('='), "State should not contain '=' padding");
}
