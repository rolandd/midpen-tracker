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
