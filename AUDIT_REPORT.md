# Codebase Audit Report: Midpen-Strava

**Date:** 2026-05-21
**Auditor:** Jules (AI Senior Full Stack Developer)
**Overall Rating:** ⭐⭐⭐⭐½ (4.5/5) - **Excellent**

## Executive Summary
The `midpen-strava` repository demonstrates a high standard of software engineering. The architecture is robust, leveraging the strengths of Rust (type safety, performance) and Svelte 5 (modern reactivity). Security is a first-class citizen with best-practice implementation of OAuth, Secret Manager, and least-privilege IAM. The project is well-positioned for open-sourcing as a showcase portfolio piece.

## Prioritized Action Items

### 🔴 Critical (Security & Data Safety)
*None found.* The repository is free of hardcoded secrets, and sensitive files are correctly ignored.

### 🟠 High (Best Practices & Safety)
1.  **Remove `impl Default` for `Config`** (`src/config.rs`)
    *   **Issue**: The `Default` implementation contains hardcoded strings like `"test_client_id"`. While `Config::from_env()` does not use these, accidental usage of `Config::default()` in a future production code path could lead to silent failures or security confusion.
    *   **Recommendation**: Remove `impl Default`. Create a named constructor `Config::test_default()` purely for the test module to make the intent explicit.

### 🟡 Medium (Code Quality & Cleanup)
1.  **Incomplete API Implementation** (`src/routes/api.rs`)
    *   **Issue**: In `get_activities`, the `else` block (when no `preserve` filter is provided) returns an empty vector with a `TODO` comment.
    *   **Recommendation**: Either implement the "get all activities" query (using Firestore pagination) or explicitly return a `400 Bad Request` if the filter is mandatory.
2.  **Unwrap Usage in Routes** (`src/routes/auth.rs`)
    *   **Issue**: `SystemTime::now()...unwrap()` is used. While effectively safe (unless the system clock is broken), it's idiomatic in Rust web services to handle all errors gracefully to avoid any risk of thread panics.
    *   **Recommendation**: Replace with `?` operator and map to `AppError::Internal`.
3.  **Project Organization**
    *   **Issue**: `generate-favicons.sh` sits in the root `web/` directory.
    *   **Recommendation**: Move to `web/scripts/` or the root `scripts/` directory to keep the source tree clean.

### 🔵 Low (Polish & Documentation)
1.  **Missing "Bad" Code**:
    *   The user requested to find "bad" code. The "worst" code found was the `TODO` in `api.rs`. The rest is remarkably clean.
2.  **Copyright Dates**:
    *   Files are marked `Copyright 2026`. (Acknowledged as intentional).

## Open Source Readiness
The project is ready for Option A (Portfolio/Showcase).
*   **Docs**: `README.md` is clear and covers architecture well.
*   **License**: MIT License is present.
*   **Build**: `justfile` makes running the project easy (`just dev-api`).

## Detailed Architecture Verification
*   **Security**: IAM roles are scoped correctly (Least Privilege). Secrets are managed via Google Secret Manager and injected at runtime.
*   **Frontend**: Verified usage of Svelte 5 Runes (`$state`, `$props`). Code is modern and clean.
*   **Backend**: Async Rust with Axum. Token encryption using Cloud KMS is a standout feature for user privacy.
