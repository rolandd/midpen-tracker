## 2024-05-18 - Missing Timeout in External API Client
**Vulnerability:** The `StravaClient` in `src/services/strava.rs` was initialized with `reqwest::Client::new()`, which does not have a default timeout.
**Learning:** Without a timeout, external HTTP requests can hang indefinitely if the remote server is slow or unresponsive. This can lead to resource exhaustion (e.g., hanging Cloud Tasks, exhausted connection pools, or blocked tokio worker threads) and potential Denial of Service (DoS) conditions.
**Prevention:** Always configure an explicit timeout when building an HTTP client for external service calls (e.g., using `reqwest::Client::builder().timeout(Duration::from_secs(10)).build()`).
