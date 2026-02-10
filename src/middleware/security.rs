// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Security headers middleware.

use axum::{extract::Request, http::HeaderValue, middleware::Next, response::Response};

/// Add security headers to all responses.
pub async fn add_security_headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();

    headers.insert(
        "X-Content-Type-Options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("X-Frame-Options", HeaderValue::from_static("DENY"));
    headers.insert(
        "Strict-Transport-Security",
        HeaderValue::from_static("max-age=31536000; includeSubDomains"),
    );
    headers.insert(
        "Content-Security-Policy",
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    headers.insert(
        "Referrer-Policy",
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(
        "Permissions-Policy",
        HeaderValue::from_static("accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()"),
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::{routing::get, Router};
    use tower::ServiceExt; // for oneshot

    #[tokio::test]
    async fn test_security_headers() {
        let app = Router::new()
            .route("/", get(|| async { "Hello" }))
            .layer(axum::middleware::from_fn(add_security_headers));

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let headers = response.headers();

        assert_eq!(headers.get("X-Content-Type-Options").unwrap(), "nosniff");
        assert_eq!(headers.get("X-Frame-Options").unwrap(), "DENY");
        assert_eq!(
            headers.get("Strict-Transport-Security").unwrap(),
            "max-age=31536000; includeSubDomains"
        );
        assert_eq!(
            headers.get("Content-Security-Policy").unwrap(),
            "default-src 'none'; frame-ancestors 'none'"
        );
        assert_eq!(headers.get("Referrer-Policy").unwrap(), "no-referrer");
        assert_eq!(
            headers.get("Permissions-Policy").unwrap(),
            "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()"
        );
    }
}
