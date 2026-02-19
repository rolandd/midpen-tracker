// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Verification tests for KMS Context-Aware Encryption (AAD).

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use midpen_tracker::services::KmsService;

#[tokio::test]
async fn test_aad_encryption_decryption() {
    // 1. Setup Mock KMS
    let kms = KmsService::new_mock();
    let plaintext = "secret_token_123";
    let athlete_id = 12345u64;
    let aad = format!("athlete_id:{}", athlete_id);
    let aad_bytes = aad.as_bytes();

    // 2. Encrypt with AAD
    let ciphertext = kms
        .encrypt(plaintext, Some(aad_bytes))
        .await
        .expect("Encryption failed");

    // 3. Decrypt with correct AAD
    let decrypted = kms
        .decrypt(&ciphertext, Some(aad_bytes))
        .await
        .expect("Decryption failed");
    assert_eq!(
        decrypted, plaintext,
        "Decrypted text should match plaintext"
    );

    // 4. Decrypt with WRONG AAD -> Should fail
    let wrong_aad = "athlete_id:99999";
    // Mock decrypt implementation throws error if AAD mismatch
    let result = kms.decrypt(&ciphertext, Some(wrong_aad.as_bytes())).await;
    assert!(result.is_err(), "Decryption with wrong AAD should fail");

    // 5. Decrypt with NO AAD -> Should fail (because it was encrypted WITH AAD)
    let result = kms.decrypt(&ciphertext, None).await;
    assert!(
        result.is_err(),
        "Decryption without AAD (when encrypted with AAD) should fail"
    );
}

#[tokio::test]
async fn test_legacy_fallback() {
    // 1. Setup Mock KMS
    let kms = KmsService::new_mock();
    let plaintext = "legacy_secret";

    // 2. Simulate Legacy Encryption (encrypt without AAD)
    // In our mock, this produces "NOAAD:..." prefixed data
    let legacy_ciphertext = kms
        .encrypt(plaintext, None)
        .await
        .expect("Encryption failed");

    let athlete_id = 12345u64;
    let aad = format!("athlete_id:{}", athlete_id);
    let aad_bytes = aad.as_bytes();

    // 3. Attempt decrypt with AAD -> Should fail first (internally) but succeed via fallback?
    // Wait, `decrypt` just fails. `decrypt_with_fallback` is the one that succeeds.

    // Direct decrypt should FAIL (expect AAD but found none)
    let result = kms.decrypt(&legacy_ciphertext, Some(aad_bytes)).await;
    assert!(
        result.is_err(),
        "Direct decrypt of legacy data with AAD expectation should fail"
    );

    // 4. Decrypt with Fallback -> Should SUCCEED
    let decrypted = kms
        .decrypt_with_fallback(&legacy_ciphertext, Some(aad_bytes))
        .await
        .expect("Fallback decryption failed");
    assert_eq!(
        decrypted, plaintext,
        "Fallback decryption should recover legacy data"
    );
}

#[tokio::test]
async fn test_legacy_data_raw() {
    // Test handling of "raw" legacy data (pre-mock update)
    let kms = KmsService::new_mock();
    let plaintext = "raw_legacy_token";
    // Manually base64 encode without prefix
    let ciphertext = BASE64.encode(plaintext);

    let athlete_id = 12345u64;
    let aad = format!("athlete_id:{}", athlete_id);
    let aad_bytes = aad.as_bytes();

    // 1. Decrypt with Fallback -> Should SUCCEED (Mock treats no prefix as legacy)
    let decrypted = kms
        .decrypt_with_fallback(&ciphertext, Some(aad_bytes))
        .await
        .expect("Fallback decryption failed for raw legacy");
    assert_eq!(decrypted, plaintext);
}
