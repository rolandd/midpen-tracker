// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Shared helpers for date/time formatting.

use chrono::{DateTime, SecondsFormat, Utc};

/// Format a UTC timestamp as RFC3339 using a `Z` suffix.
pub fn format_utc_rfc3339(date: DateTime<Utc>) -> String {
    date.to_rfc3339_opts(SecondsFormat::Secs, true)
}
