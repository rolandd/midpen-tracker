// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Shared helpers for date/time formatting.

use jiff::Timestamp;

/// Format a UTC timestamp as RFC3339 using a `Z` suffix and second precision.
pub fn format_utc_rfc3339(timestamp: Timestamp) -> String {
    timestamp.strftime("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Parse an RFC3339 timestamp.
pub fn parse_rfc3339(input: &str) -> Result<Timestamp, jiff::Error> {
    input.parse::<Timestamp>()
}

/// Validate that a string is a valid RFC3339 date (for validator crate).
pub fn validate_rfc3339(date: &str) -> std::result::Result<(), validator::ValidationError> {
    parse_rfc3339(date)
        .map(|_| ())
        .map_err(|_| validator::ValidationError::new("invalid_rfc3339"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_utc_rfc3339_truncates_subseconds() {
        let ts = parse_rfc3339("2024-06-19T15:22:45.123456789+00:00").unwrap();
        assert_eq!(format_utc_rfc3339(ts), "2024-06-19T15:22:45Z");
    }

    #[test]
    fn test_format_utc_rfc3339_normalizes_offset() {
        let ts = parse_rfc3339("2024-06-19T17:22:45+02:00").unwrap();
        assert_eq!(format_utc_rfc3339(ts), "2024-06-19T15:22:45Z");
    }

    #[test]
    fn test_validate_rfc3339() {
        assert!(validate_rfc3339("2024-01-15T12:00:00Z").is_ok());
        assert!(validate_rfc3339("2024-01-15T12:00:00+00:00").is_ok());
        assert!(validate_rfc3339("not-a-date").is_err());
        assert!(validate_rfc3339("2024-01-15").is_err());
    }
}
