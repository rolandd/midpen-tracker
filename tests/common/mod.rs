// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

use midpen_strava::db::FirestoreDb;

/// Check if emulator is available via environment variable.
pub fn emulator_available() -> bool {
    std::env::var("FIRESTORE_EMULATOR_HOST").is_ok()
}

/// Skip test with message if emulator not available.
#[macro_export]
macro_rules! require_emulator {
    () => {
        if !crate::common::emulator_available() {
            eprintln!("⚠️  Skipping: FIRESTORE_EMULATOR_HOST not set");
            return;
        }
    };
}

/// Create a test database connection.
pub async fn test_db() -> FirestoreDb {
    FirestoreDb::new("test-project")
        .await
        .expect("Failed to connect to Firestore emulator")
}
