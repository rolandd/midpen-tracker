## 2025-02-28 - [Missing Timeout on External API Client]
**Vulnerability:** External HTTP client `reqwest::Client` used for the Strava API was instantiated using `reqwest::Client::new()`, which does not have a default timeout.
**Learning:** This could lead to resource exhaustion (Denial of Service) if the external API hangs indefinitely, as it ties up internal resources (e.g., threads, connection pool).
**Prevention:** Always use `reqwest::Client::builder().timeout(...)` to configure explicit timeouts for all external HTTP clients.
