## 2026-01-21 - Secure Cloud Tasks Handlers on Cloud Run
**Vulnerability:** Publicly accessible task handler endpoints allowed potential DoS or data corruption.
**Learning:** On Cloud Run, the `X-CloudTasks-QueueName` header is stripped from external requests but preserved for internal Cloud Tasks requests.
**Prevention:** Enforce the presence of this header in task handlers to ensure requests originate from Cloud Tasks without managing secrets.

## 2026-05-23 - Unverified Webhook Events
**Vulnerability:** Strava webhooks lack signature verification, allowing attackers to spoof events (e.g., user deletion).
**Learning:** When cryptographic verification is impossible (vendor limitation), verify the *state* of the resource against the vendor's API before acting on destructive events.
**Prevention:** For `delete` or `deauthorize` events, query the source API (e.g., `get_activity` or `get_valid_token`) to confirm the event is legitimate.

## 2026-06-15 - Stateless OAuth State insufficient for CSRF
**Vulnerability:** OAuth state parameter only contained a signed timestamp/url, preventing tampering but allowing replay attacks (Login CSRF) from other sessions.
**Learning:** Signed stateless tokens verify the *server* generated them, but not *which client* requested them.
**Prevention:** Bind the OAuth state to the browser session using a short-lived HttpOnly nonce cookie matched against the state payload.

## 2026-07-20 - Open Redirect in OAuth Flow
**Vulnerability:** Open Redirect in OAuth initialization allowed attackers to redirect users to malicious sites after login via `redirect_uri` parameter.
**Learning:** Validating redirects against a trusted base URL requires careful handling of trailing slashes to prevent prefix attacks (e.g. `site.com.evil.com`).
**Prevention:** Strictly validate `redirect_uri` against the configured frontend origin, ensuring directory boundary checks.

## 2026-07-20 - Token Caching Bypasses Deauthorization Checks
**Vulnerability:** The system relied on cached access tokens to "verify" deauthorization webhooks. If a token was cached (valid timestamp), the system assumed the user was still authorized and treated the deauth webhook as fake/spoofed, failing to delete user data.
**Learning:** Local token caches (based on timestamps) are not authoritative for revocation status. A token can be valid locally but revoked upstream.
**Prevention:** When verifying destructive events (like deauthorization), bypass the local cache and force a live API call (e.g., `get_athlete`) to confirm the token's status with the provider.

## 2026-02-03 - Missing Security Headers in API
**Vulnerability:** API endpoints lacked standard security headers (HSTS, X-Frame-Options, etc.), increasing risk of man-in-the-middle or clickjacking attacks if the API is accessed directly.
**Learning:** Even purely JSON APIs benefit from security headers like HSTS (to enforce HTTPS) and CSP (to prevent content sniffing or framing if accessed by a browser).
**Prevention:** Implement a global middleware layer that injects strict security headers on all responses, tailored for an API-only environment (e.g. `default-src 'none'`).

## 2026-02-04 - Integer Underflow in Pagination
**Vulnerability:** API pagination logic calculated offset as `(page - 1) * limit`. When `page=0`, this caused an integer underflow panic in debug mode and potentially huge offsets in release mode.
**Learning:** Rust's integer types do not implicitly handle underflow safely in arithmetic expressions unless checked (e.g. `saturating_sub`). Input validation is critical before arithmetic.
**Prevention:** Explicitly validate pagination parameters (e.g. `page < 1`) and return clear errors (400 Bad Request) rather than silent clamping or unsafe math.

## 2026-02-04 - Integer Overflow in Pagination
**Vulnerability:** API pagination logic calculated offset as `(page - 1) * limit` or `next_page + 1` without overflow checks. Large inputs (`u32::MAX`) caused panics (DoS) or logic errors (wrapping to 0).
**Learning:** Rust arithmetic panics on overflow in debug builds and wraps in release builds. Both are dangerous for security (DoS or Logic Flaw).
**Prevention:** Use `checked_mul`, `checked_add`, or safe casts (e.g. `u64`) for all arithmetic operations involving user-controlled inputs. Explicitly handle overflows with `400 Bad Request`.

## 2026-08-15 - Unauthenticated Resource Consumption in Webhook Handler
**Vulnerability:** The webhook endpoint parsed JSON payloads before validating the path secret, allowing attackers to trigger CPU-intensive parsing on invalid requests.
**Learning:** Axum extractors run before the handler body. Using `Json<T>` as an argument implicitly parses the body, exposing the application to DoS attacks on public endpoints.
**Prevention:** For endpoints protected by path secrets or headers, accept the raw body (e.g., `Bytes`), validate the secret first, and then parse the payload manually.

## 2026-06-03 - Input Validation for Database Queries
**Vulnerability:** API query parameters (`preserve`, `after`) were passed directly to Firestore queries without validation. While not an injection risk (due to Firestore's query builder), it allowed potentially unbounded input strings and invalid date formats.
**Learning:** Even when using safe query builders (NoSQL), input validation is crucial for defense-in-depth, preventing performance degradation (DoS via large inputs) and logic errors.
**Prevention:** Always validate all user inputs at the API boundary (length, format, type) before passing them to internal services or databases.
