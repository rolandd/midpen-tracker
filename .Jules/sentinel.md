## 2026-08-16 - Missing Timeout on External API Client
**Vulnerability:** The `reqwest::Client` used for the Strava API was instantiated without an explicit timeout (`reqwest::Client::new()`).
**Learning:** Default HTTP clients often have no timeout or extremely long timeouts. If the external API becomes slow or unresponsive, the application's threads or async tasks will hang indefinitely, leading to resource exhaustion and Denial of Service (DoS).
**Prevention:** Always configure explicit timeouts when building HTTP clients for external services using `.timeout(Duration::from_secs(X))` on the builder.
