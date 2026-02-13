// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

use midpen_tracker::services::kms::{decrypt_tokens, encrypt_tokens};
use midpen_tracker::services::KmsService;

#[tokio::test]
async fn test_kms_mock_aad_logic() {
    let kms = KmsService::new_mock();
    let plaintext = "secret_access_token";
    let athlete_id = 12345u64;
    let aad = athlete_id.to_be_bytes();

    // 1. Encrypt with AAD
    let ciphertext = kms
        .encrypt(plaintext, Some(&aad))
        .await
        .expect("Encryption failed");

    // Mock format check: MOCK_V1:Base64(AAD):Base64(PT)
    assert!(ciphertext.starts_with("MOCK_V1:"));

    // 2. Decrypt with Correct AAD
    let decrypted = kms
        .decrypt(&ciphertext, Some(&aad))
        .await
        .expect("Decryption with correct AAD failed");
    assert_eq!(decrypted, plaintext);

    // 3. Decrypt with Wrong AAD
    let wrong_aad = 99999u64.to_be_bytes();
    let err = kms
        .decrypt(&ciphertext, Some(&wrong_aad))
        .await
        .expect_err("Decryption with wrong AAD should fail");
    assert!(err.to_string().contains("Mock KMS AAD mismatch"));

    // 4. Decrypt with No AAD (Should fail because token HAS AAD)
    // In mock implementation: stored AAD="12345", provided AAD="" -> Mismatch
    let err = kms
        .decrypt(&ciphertext, None)
        .await
        .expect_err("Decryption with no AAD should fail for AAD-protected token");
    assert!(err.to_string().contains("Mock KMS AAD mismatch"));
}

#[tokio::test]
async fn test_kms_fallback_logic() {
    let kms = KmsService::new_mock();
    let plaintext = "legacy_token";
    let athlete_id = 12345u64;
    let aad = athlete_id.to_be_bytes();

    // 1. Create a "Legacy" token (encrypted with No AAD)
    let legacy_ciphertext = kms
        .encrypt(plaintext, None) // No AAD
        .await
        .expect("Legacy encryption failed");

    // 2. Try decrypting using `decrypt_or_fallback` with AAD
    // It should try AAD -> Fail -> Fallback -> Success
    let decrypted = kms
        .decrypt_or_fallback(&legacy_ciphertext, &aad)
        .await
        .expect("Fallback decryption failed");
    assert_eq!(decrypted, plaintext);

    // 3. Create a "New" token (encrypted WITH AAD)
    let new_ciphertext = kms
        .encrypt(plaintext, Some(&aad))
        .await
        .expect("New encryption failed");

    // 4. Try decrypting using `decrypt_or_fallback` with Correct AAD
    // It should try AAD -> Success
    let decrypted_new = kms
        .decrypt_or_fallback(&new_ciphertext, &aad)
        .await
        .expect("New token decryption failed");
    assert_eq!(decrypted_new, plaintext);

    // 5. Try decrypting using `decrypt_or_fallback` with WRONG AAD
    // It should try AAD (wrong) -> Fail -> Fallback (no AAD) -> Fail (because token has AAD)
    let wrong_aad = 99999u64.to_be_bytes();
    let err = kms
        .decrypt_or_fallback(&new_ciphertext, &wrong_aad)
        .await
        .expect_err("Decryption with wrong AAD should fail even with fallback");

    // The error comes from fallback attempt failing
    assert!(err.to_string().contains("Mock KMS AAD mismatch"));
}

#[tokio::test]
async fn test_token_helpers_integration() {
    let kms = KmsService::new_mock();
    let access = "access_token_123";
    let refresh = "refresh_token_456";
    let athlete_id = 12345u64;

    // 1. Encrypt tokens with AAD (via helper)
    let (enc_access, enc_refresh) = encrypt_tokens(&kms, access, refresh, athlete_id)
        .await
        .expect("Encrypt tokens helper failed");

    // 2. Decrypt tokens with correct ID (via helper)
    let (dec_access, dec_refresh) = decrypt_tokens(&kms, &enc_access, &enc_refresh, athlete_id)
        .await
        .expect("Decrypt tokens helper failed");

    assert_eq!(dec_access, access);
    assert_eq!(dec_refresh, refresh);

    // 3. Decrypt tokens with wrong ID (via helper)
    let wrong_id = 99999u64;
    let err = decrypt_tokens(&kms, &enc_access, &enc_refresh, wrong_id)
        .await
        .expect_err("Decrypt tokens with wrong ID should fail");

    // Error message might vary depending on which token fails first or if fallback masks it
    // But it should definitely fail.
    assert!(err.to_string().contains("Mock KMS AAD mismatch"));
}
