## 2026-01-21 - Secure Cloud Tasks Handlers on Cloud Run
**Vulnerability:** Publicly accessible task handler endpoints allowed potential DoS or data corruption.
**Learning:** On Cloud Run, the `X-CloudTasks-QueueName` header is stripped from external requests but preserved for internal Cloud Tasks requests.
**Prevention:** Enforce the presence of this header in task handlers to ensure requests originate from Cloud Tasks without managing secrets.

## 2026-05-23 - Unverified Webhook Events
**Vulnerability:** Strava webhooks lack signature verification, allowing attackers to spoof events (e.g., user deletion).
**Learning:** When cryptographic verification is impossible (vendor limitation), verify the *state* of the resource against the vendor's API before acting on destructive events.
**Prevention:** For `delete` or `deauthorize` events, query the source API (e.g., `get_activity` or `get_valid_token`) to confirm the event is legitimate.
