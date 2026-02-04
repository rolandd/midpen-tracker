// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use midpen_tracker::config::Config;
use midpen_tracker::db::FirestoreDb;
use midpen_tracker::routes::create_router;
use midpen_tracker::services::{KmsService, PreserveService, StravaService, TasksService};
use midpen_tracker::AppState;
use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Create a test JWT token.
#[allow(dead_code)]
pub fn create_test_jwt(athlete_id: u64, signing_key: &[u8]) -> String {
    #[derive(Serialize)]
    struct Claims {
        sub: String,
        exp: usize,
        iat: usize,
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = Claims {
        sub: athlete_id.to_string(),
        exp: now + 86400 * 30,
        iat: now,
    };

    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(signing_key),
    )
    .unwrap()
}

/// Check if emulator is available via environment variable.
#[allow(dead_code)]
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
#[allow(dead_code)]
pub async fn test_db() -> FirestoreDb {
    FirestoreDb::new("test-project")
        .await
        .expect("Failed to connect to Firestore emulator")
}

/// Create a mock database connection (offline).
#[allow(dead_code)]
pub fn test_db_offline() -> FirestoreDb {
    FirestoreDb::new_mock()
}

/// Create a test app with offline mock dependencies.
/// Returns the router and the shared state.
#[allow(dead_code)]
pub fn create_test_app() -> (axum::Router, Arc<AppState>) {
    let config = Config::test_default();
    let db = test_db_offline();
    let preserve_service = PreserveService::default();
    let tasks_service = TasksService::new(&config.gcp_project_id, &config.gcp_region);

    let kms = KmsService::new_mock();
    let token_cache = Arc::new(dashmap::DashMap::new());
    let refresh_locks = Arc::new(dashmap::DashMap::new());

    let strava_service = StravaService::new(
        config.strava_client_id.clone(),
        config.strava_client_secret.clone(),
        db.clone(),
        kms,
        token_cache,
        refresh_locks,
    );

    let state = Arc::new(AppState {
        config,
        db,
        preserve_service,
        tasks_service,
        strava_service,
    });

    (create_router(state.clone()), state)
}
