# Codebase Audit Report

**Date:** 2026-05-21
**Auditor:** Jules (AI Senior Full Stack Developer)
**Scope:** Full Codebase (Infrastructure, Backend, Frontend)

## 1. Executive Summary

**Overall Quality Rating:** **A- (Excellent)**

This repository represents a high-quality, professional-grade portfolio project. The architecture demonstrates advanced knowledge of cloud-native patterns (Serverless, Event-Driven, Async Processing) and modern frameworks (Rust/Axum, SvelteKit/Svelte 5).

**Open Source Readiness:** **Ready**.
No hardcoded secrets were found. The codebase is clean, well-documented, and follows standard conventions. The project is safe to be made public on GitHub.

### Key Strengths
*   **Security First:** Uses Google Secret Manager, Cloud KMS for token encryption, and robust IAM roles.
*   **Architecture:** Async processing with Cloud Tasks prevents timeouts and handles rate limiting elegantly.
*   **Privacy:** "Delete User" flow is robust, handling race conditions and prioritizing data removal.
*   **Modern Stack:** Uses the very latest tools (Svelte 5 Runes, Tailwind 4, Axum 0.8).

---

## 2. Detailed Findings

### 2.1 Infrastructure (`infra/`)
The Terraform configuration is exemplary.
*   **Security:** Follows least-privilege principles. The Cloud Run service account has specific roles (`datastore.user`, `cloudtasks.enqueuer`) rather than broad editor permissions.
*   **Secrets:** Secrets are injected via Secret Manager references, not hardcoded in environment variables.
*   **Reliability:** Cloud Tasks queue is configured with safe rate limits (6/min) to respect Strava's API quotas.

### 2.2 Backend (`src/`)
The Rust backend is idiomatic and robust.
*   **Token Security:** Strava access/refresh tokens are **encrypted at rest** using Cloud KMS. This is a production-grade security feature rarely seen in portfolio projects.
*   **Task Verification:** Internal endpoints (`/tasks/*`) correctly verify the `X-CloudTasks-QueueName` header to prevent unauthorized access.
*   **Error Handling:** The `AppError` enum ensures internal implementation details (database errors) are logged but hidden from API clients.
*   **Logic:** The "recursive backfill" pattern (fetching one page, then queuing a task for the next) is a smart way to handle large datasets without hitting HTTP timeout limits.

### 2.3 Frontend (`web/`)
The frontend uses the latest Svelte ecosystem features.
*   **Modern Syntax:** Correct usage of Svelte 5 runes (`$props`, `$state`).
*   **Architecture:** Separation of concerns between UI components and API logic (`lib/api.ts`).
*   **CSP:** Content Security Policy is present, though it relies on `unsafe-inline` (common for simple SPAs but a minor weakness).

---

## 3. Action Items

### Critical Priority (Immediate Fixes)
*   *None found.* The codebase is secure and functional.

### High Priority (Recommended before Public Release)
*   **Sign OAuth State Parameter:** The OAuth `state` parameter currently encodes data (frontend URL) but is not signed.
    *   *Risk:* A malicious actor could theoretically tamper with the redirect URL.
    *   *Fix:* Add an HMAC signature to the `state` string using a secret key.

### Medium Priority (Improvements)
*   **CSP Hardening:** The `content-security-policy` in `app.html` uses `'unsafe-inline'` for scripts and styles.
    *   *Fix:* Use nonces or hashes for scripts to strictly prevent XSS.
*   **Prerender Config:** `svelte.config.js` sets `entries: ['*']`. If the app grows to include dynamic links (e.g., `/activity/123`), the build might fail if it tries to prerender them.
    *   *Fix:* Explicitly define entry points or disable prerendering for dynamic routes.
*   **LocalStorage for Tokens:** Auth tokens are stored in `localStorage`.
    *   *Risk:* Susceptible to XSS attacks (if an attacker can run JS, they can read the token).
    *   *Fix:* Storing tokens in `HttpOnly` cookies is safer, though it requires more complex CORS/SameSite setup with a separate backend domain. For a portfolio, the current approach is acceptable but worth noting.

### Low Priority (Nitpicks)
*   **Config Naming:** In `src/config.rs`, the method `fetch_secret` just reads from environment variables (because Cloud Run injects them). The naming implies an API call. Renaming to `get_secret_from_env` would be clearer.
*   **GeoJSON Loading:** `PreserveService` loads the entire boundary file into memory. For this dataset, it's fine, but for larger datasets, a spatial database (PostGIS) or streaming parser would be better.

---

## 4. Conclusion

This is an impressive codebase that successfully balances complexity with maintainability. It demonstrates "Senior Engineer" traits by prioritizing security, handling failure states (retries, rate limits), and using strong typing throughout the stack. It is ready for public release.
