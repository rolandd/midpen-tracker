# Cloud Tasks OIDC Verification Design

Date: 2026-02-11
Status: Accepted
Scope: Add first-class OIDC verification for `/tasks/*` routes, with Google JWKS discovery/fetch and in-process certificate caching.

## 1. Goals

1. Enforce identity-based authentication for Cloud Tasks callbacks.
2. Keep existing queue-name header validation as secondary defense.
3. Validate JWTs offline using cached Google public certs/JWKS.
4. Avoid introducing new dependencies; use crates already in the repo.
5. Keep runtime behavior predictable under key rotation and transient Google endpoint failures.

## 2. Non-Goals

1. Building a separate external JWKS service.
2. Changing Cloud Tasks payload formats.
3. Reworking general auth/JWT flows for end-user sessions.

## 3. Current State (as of 2026-02-11)

1. Task handlers are mounted on public routes in `src/routes/mod.rs`.
2. Task handlers in `src/routes/tasks.rs` currently gate on `x-cloudtasks-queuename`.
3. `TasksService` already configures OIDC token generation for tasks (`set_oidc_token(...)`).
4. No middleware currently verifies `Authorization: Bearer <id_token>` on `/tasks/*`.

## 4. Design Constraints

1. Reuse existing crates:
- `reqwest` for HTTP fetch.
- `serde`/`serde_json` for discovery/JWKS parsing.
- `jsonwebtoken` for JWT header parsing and signature/claim verification.
- `tokio` synchronization primitives for cache concurrency.
- `tracing` for observability.

2. No added dependencies.

## 5. Proposed Architecture

## 5.1 New module layout

1. `src/middleware/tasks_auth.rs`
- Axum middleware for `/tasks/*`.
- Performs:
  - queue header check (`x-cloudtasks-queuename == ACTIVITY_QUEUE_NAME`)
  - OIDC token extraction + verification

2. `src/services/google_oidc.rs`
- Encapsulates all OIDC/JWKS logic.
- Public API:
  - `verify_cloud_tasks_token(auth_header: Option<&HeaderValue>) -> Result<VerifiedTaskPrincipal, OidcError>`

3. `src/services/mod.rs`
- Export `GoogleOidcVerifier`.

4. `src/lib.rs` + `AppState`
- Add shared verifier instance to application state.

## 5.2 Router wiring

1. Keep `tasks::routes()` under public routing group (no user JWT requirement).
2. Apply route-layer middleware only to `/tasks/*` router:
- `middleware::from_fn_with_state(state.clone(), require_tasks_auth)`.
3. Remove duplicated header-check blocks from each task handler once middleware is active.

This centralizes auth and reduces copy/paste divergence.

## 6. Token Verification Rules

For every `/tasks/*` request:

1. Require `Authorization` header with `Bearer <JWT>`.
2. Decode JWT header and require:
- `alg == RS256`
- `kid` present
3. Resolve signing key by `kid` from JWKS cache.
4. Verify JWT signature + standard claims:
- `iss` in:
  - `https://accounts.google.com`
  - `accounts.google.com`
- `aud == config.api_url` (canonicalized, no trailing slash)
- `exp` valid
- `iat`/`nbf` valid (with small configured leeway, e.g. 60s)
5. Verify service identity claim:
- `email == format!("midpen-tracker-api@{}.iam.gserviceaccount.com", config.gcp_project_id)`
- `email_verified == true` (if present; if absent, reject for strictness)
6. Require queue header match as secondary check.

If any identity check fails: return `403 FORBIDDEN`.

## 7. Google JWKS Source and Caching

## 7.1 Source of truth

Preferred flow:

1. Fetch OIDC discovery doc from:
- `https://accounts.google.com/.well-known/openid-configuration`
2. Read `jwks_uri`.
3. Fetch JWKS from that URI.

Fallback if discovery fails but we already have a known URI configured:

1. Use last known `jwks_uri` from cache.
2. If none exists, use default constant:
- `https://www.googleapis.com/oauth2/v3/certs`

## 7.2 Cache model

`GoogleOidcVerifier` keeps in-process cache:

1. `discovery_cache`
- `jwks_uri: String`
- `expires_at: Instant`

2. `jwks_cache`
- `keys_by_kid: HashMap<String, DecodingKey>`
- `expires_at: Instant`
- optional metadata: `etag`

3. synchronization
- `RwLock` for read-mostly access.
- `Mutex` for singleflight refresh (one concurrent refresh per process).

## 7.3 Expiry policy

1. Respect `Cache-Control: max-age=<N>` from HTTP responses.
2. If header missing/unparseable, use conservative default TTL (e.g. 300 seconds).
3. Refresh triggers:
- cache expired
- `kid` not found (forced refresh once)

## 7.4 Failure policy

1. If JWT is invalid/signature fails/claims mismatch: `403`.
2. If key refresh/discovery fetch fails and no usable unexpired key exists: `500`.
- Reason: transient infra failure should cause Cloud Tasks retry.
3. If fetch fails but cache is still unexpired and contains needed `kid`, continue with cached key.

## 8. Configuration Changes (Minimal)

No new environment variables for rollout or claim matching.

Use existing `Config` fields:

1. `api_url`
- Used as expected `aud` (canonicalized, no trailing slash).

2. `gcp_project_id`
- Used to derive expected invoker service account email:
  `midpen-tracker-api@<project>.iam.gserviceaccount.com`.

3. Existing environment split remains:
- Production: strict OIDC validation is always on for `/tasks/*`.
- Tests/local: tests can use injected verifier doubles or deterministic test keys; no runtime feature flag in production path.

## 9. Detailed Request Flow

1. Request hits `/tasks/*`.
2. `require_tasks_auth` middleware runs.
3. Middleware validates queue header.
4. Middleware parses bearer token.
5. Middleware calls `state.google_oidc_verifier.verify_cloud_tasks_token(...)`.
6. On success, continue to handler; on failure, return status as per policy.
7. Handler executes business logic with no per-handler auth duplication.

## 10. Internal APIs (proposed)

```rust
// src/services/google_oidc.rs
pub struct GoogleOidcVerifier { /* cache + http client + expected claims */ }

pub struct VerifiedTaskPrincipal {
    pub email: String,
    pub subject: String,
    pub audience: String,
}

pub enum OidcError {
    Forbidden(String), // invalid/missing token, claim mismatch
    Transient(String), // discovery/JWKS fetch problems
}

impl GoogleOidcVerifier {
    pub fn new(config: &Config) -> anyhow::Result<Self>;

    pub async fn verify_cloud_tasks_token(
        &self,
        auth_header: Option<&axum::http::HeaderValue>,
    ) -> Result<VerifiedTaskPrincipal, OidcError>;
}
```

```rust
// src/middleware/tasks_auth.rs
pub async fn require_tasks_auth(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode>;
```

## 11. Data Structures (serde)

Use simple local structs:

```rust
#[derive(Deserialize)]
struct OpenIdConfig {
    jwks_uri: String,
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct Jwk {
    kid: String,
    kty: String, // require "RSA"
    alg: Option<String>,
    n: String,
    e: String,
    #[serde(rename = "use")]
    use_: Option<String>,
}

#[derive(Deserialize)]
struct GoogleIdTokenClaims {
    iss: String,
    aud: String,
    sub: String,
    exp: usize,
    iat: Option<usize>,
    nbf: Option<usize>,
    email: Option<String>,
    email_verified: Option<bool>,
}
```

Decoding key creation uses existing `jsonwebtoken`:

- `DecodingKey::from_rsa_components(&jwk.n, &jwk.e)`

## 12. Logging and Observability

Add structured logs at verification boundary:

1. Debug:
- cache hit/miss for `kid`
- JWKS refresh start/success

2. Warn:
- missing/invalid bearer token
- claim mismatch (aud/iss/email/email_verified)

3. Info (temporary rollout visibility):
- observed `email_verified` value from Cloud Tasks tokens
- observed token service-account email (`email`)

4. Error:
- discovery/JWKS fetch failures
- JSON parse errors for cert endpoints

Do not log raw JWTs.

## 13. Testing Plan

## 13.1 Unit tests (`src/services/google_oidc.rs`)

1. `parse_cache_control_max_age` cases.
2. `bearer` parsing edge cases.
3. Claims validation:
- wrong audience
- wrong issuer
- missing/false `email_verified`
- mismatched service account email
4. Cache behavior:
- cache hit path
- expired cache triggers refresh
- unknown `kid` forces one refresh

Use static RSA test fixtures and JWKS JSON in test constants; no network.

## 13.2 Middleware tests (`tests/tasks_security_tests.rs`)

1. missing Authorization => `FORBIDDEN`.
2. wrong queue header => `FORBIDDEN`.
3. valid token + valid queue header => request reaches handler.
4. transient verifier failure => `INTERNAL_SERVER_ERROR` (retryable).

Do not rely on runtime feature flags for coverage; verify both success and failure paths directly with test fixtures and mocked fetch behavior.

## 14. Rollout Plan

1. Ship verifier module + middleware with strict production validation enabled immediately.
2. Remove per-handler queue-header checks after middleware is wired (or keep briefly as temporary defense-in-depth).
3. If production rejects occur, debug misconfiguration/token issues directly rather than toggling off validation.

## 15. Risks and Mitigations

1. Risk: key endpoint outage blocks task processing.
- Mitigation: honor cache TTL, return 500 only when no valid cached key exists.

2. Risk: misconfigured `API_URL` or project identity causes all tasks to fail.
- Mitigation: startup validation of derived expected claims + explicit logging of expected audience and expected service account email (non-secret values).

3. Risk: duplicate auth checks in middleware/handlers drift.
- Mitigation: centralize in middleware and simplify handlers.

## 16. Open Decisions

1. Should missing `email_verified` be treated as hard fail or allow when `email` matches?
2. Do we want to include optional `sub` pinning for stricter identity matching?

## 17. Summary

Implement OIDC verification as a dedicated middleware + service module, with in-process JWKS caching and singleflight refresh. This gives a stronger, identity-based control plane for `/tasks/*` while reusing current crates and preserving Cloud Tasks retry semantics on transient key-fetch failures.
