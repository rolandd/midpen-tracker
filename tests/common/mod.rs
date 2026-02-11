// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

use jsonwebtoken::{encode, Algorithm, DecodingKey, EncodingKey, Header};
use midpen_tracker::config::Config;
use midpen_tracker::db::FirestoreDb;
use midpen_tracker::routes::create_router;
use midpen_tracker::services::{
    GoogleOidcVerifier, KmsService, PreserveService, StravaService, TasksService,
};
use midpen_tracker::AppState;
use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub const TEST_TASKS_OIDC_KID: &str = "tasks-test-kid";

const TEST_TASKS_OIDC_PRIVATE_KEY_PEM: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCm8LcTFrxzOwtY
iMmKSnyDRTaAtfpEPh9tQl/SlrmtF0sr2aofCg7VEzJUMchoLLtl6QvB6ZkQvxjy
kWk8TDVlxNnaK4+dXTFvZW2X3dNEaq8nlFGcMNDMY43IPFTcNtmfIQf/67nDG8Ms
idgBsVbLWQW2SRQwnWOie2+L2Ia+WAwZR0JpmRe0G85Yg/h6G5F75CJKQuJ+DjKv
7vDkOVeutwtkmyTYpE8NihlFcS71bDpfmnU9RGv5BZrjn/R72KBhNWiI+K1CrKyx
Xf6qcFBwrlgjLzlpeHOquAVjM9tWf1S9MxftuPdhfqdnv6HxtYQY4/a2EaKz96NQ
Vsn348m/AgMBAAECggEAIlDF2SspxqgC74qCnyYrjRJLn06I6ME6VYu4xcGM3ks0
/QkLosC/Bsag6FSpZNyZEtxCOfSKLbqSZr5EOwxRR5+cxm+A4HCZKlRXoEmNVXl2
krS2x9vy96iZKulX6kJuHns8OTub3kLVLlERcVuiQz+D4JIKD+oyYmJsLftuyy5Z
4BdH0axgrxmbk4HgimsRnidx9Vztu5o+9pbD7j7SJmvoiikok/XEfnXe7DJ7RSyd
AKNTbSzpncmQt3CfP6aRSzl6CIpywVjr0RGZ4dmweKb/jNtUgJ1hZknl4Jt1neOh
rygGaAveWAFLvne5v9KueaTgIskNWmpJVcX6XJ+cbQKBgQDf16F1U4FkSneUIvXC
i7kPbhu+uTZCgyoQEmlpv/w7QVTpkpLwkjOP6Mm2CJkjbL5FHQUAfBv/jV4OA+g+
7xb1SUUcLSSdw4KuUPNZzkxSofITW4d3MqfLGOrf6scpsBirt1r8nmpYBy0cE/Wq
q48o85aG7NAkdoSahYa8Ub6MAwKBgQC+7F5Nwcz5U06ugYCQx4CeItsiIhU9sg2A
8LQHLyHX9s2RF3cterOquG1RYgtVwck28IDWJTF0NT7G17rZwUjWmuI9l81sPsJd
4uSuxV0xsGDqwWywtC49JsLG66SzhlGwXx3dpLiCfVHOOP7DCJwTD5NG9HmKQpZH
wf+zcu7ElQKBgFpieJimimXTx+syHqhawPQhEuT1Zpp+2ho5RQVld1T58W6LN/ga
IOXoKqLtX+C1BTNlH2LtumR7UdI486uN4WhUGKri85kcnAUFPO4zZhArwlLcr5uL
AcP5oMWfyKHlsGCOHhhJY0l+RFHFIXqz4Y+4pDyBHR7MGIlIh3o9S8K/AoGAB02t
cdYHDEaWjPBhRaiMEACPV2fsXhbQk20hxeCUr9k+Fd3K7k9yTgaOD/3rJxWpp9Nd
alOz55kd1Kdt+2R8b9Eu9GI5NnnUH5lNXC4qmXsAyhoqGTxbRHWWH9vlygRKXa/V
yaPCdyNqHLRrcnSC2+vNm3pAp/xSGV6fdHLiFV0CgYA7Hvf8V4YNRxjre0v0v/HQ
8I/P7P1KjZcvy9zVeX8E6Q2sUMXjNydJ6MPpWNBEW9y92zuQg0n1yS70QYVGo5Ek
XjPZAgSr32/fUkWKeLzFJity3w1wUmSY431SsKnphCbPw4wY39Tkx8SCdsufgHlo
9kbC0aJGAEokaTRIJ+odMQ==
-----END PRIVATE KEY-----"#;

const TEST_TASKS_OIDC_PUBLIC_KEY_PEM: &str = r#"-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEApvC3Exa8czsLWIjJikp8
g0U2gLX6RD4fbUJf0pa5rRdLK9mqHwoO1RMyVDHIaCy7ZekLwemZEL8Y8pFpPEw1
ZcTZ2iuPnV0xb2Vtl93TRGqvJ5RRnDDQzGONyDxU3DbZnyEH/+u5wxvDLInYAbFW
y1kFtkkUMJ1jontvi9iGvlgMGUdCaZkXtBvOWIP4ehuRe+QiSkLifg4yr+7w5DlX
rrcLZJsk2KRPDYoZRXEu9Ww6X5p1PURr+QWa45/0e9igYTVoiPitQqyssV3+qnBQ
cK5YIy85aXhzqrgFYzPbVn9UvTMX7bj3YX6nZ7+h8bWEGOP2thGis/ejUFbJ9+PJ
vwIDAQAB
-----END PUBLIC KEY-----"#;

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

/// Create a test Cloud Tasks OIDC JWT token.
#[allow(dead_code)]
pub fn create_test_tasks_oidc_jwt(config: &Config) -> String {
    #[derive(Serialize)]
    struct OidcClaims {
        iss: String,
        aud: String,
        sub: String,
        exp: usize,
        iat: usize,
        nbf: usize,
        email: String,
        email_verified: bool,
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(TEST_TASKS_OIDC_KID.to_string());

    let expected_email = format!(
        "midpen-tracker-api@{}.iam.gserviceaccount.com",
        config.gcp_project_id
    );

    let claims = OidcClaims {
        iss: "https://accounts.google.com".to_string(),
        aud: config.api_url.trim_end_matches('/').to_string(),
        sub: "tasks-test-subject".to_string(),
        exp: now + 300,
        iat: now,
        nbf: now.saturating_sub(1),
        email: expected_email,
        email_verified: true,
    };

    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(TEST_TASKS_OIDC_PRIVATE_KEY_PEM.as_bytes()).unwrap(),
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
    create_test_app_with_frontend_url("http://localhost:5173")
}

/// Create a test app with a custom frontend URL.
#[allow(dead_code)]
pub fn create_test_app_with_frontend_url(frontend_url: &str) -> (axum::Router, Arc<AppState>) {
    let config = Config::test_default();
    let config = Config {
        frontend_url: frontend_url.to_string(),
        ..config
    };
    let db = test_db_offline();
    let preserve_service = PreserveService::default();
    let tasks_service = TasksService::new(&config.gcp_project_id, &config.gcp_region);
    let oidc_decoding_key = DecodingKey::from_rsa_pem(TEST_TASKS_OIDC_PUBLIC_KEY_PEM.as_bytes())
        .expect("test OIDC public key must be valid");
    let google_oidc_verifier = Arc::new(
        GoogleOidcVerifier::new_with_static_key(&config, TEST_TASKS_OIDC_KID, oidc_decoding_key)
            .expect("test OIDC verifier should initialize"),
    );

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
        google_oidc_verifier,
        strava_service,
    });

    (create_router(state.clone()), state)
}

#[allow(dead_code)]
pub fn parse_time(rfc3339: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .unwrap()
        .with_timezone(&chrono::Utc)
}
