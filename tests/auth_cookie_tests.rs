// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Auth cookie attribute tests.
//!
//! These tests verify cookie removal attributes on logout match the creation
//! attributes for localhost and production-style domains.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    response::Response,
};
use tower::ServiceExt;

mod common;

fn set_cookie_headers(response: &Response) -> Vec<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().unwrap().to_string())
        .collect()
}

fn find_cookie(headers: &[String], name: &str) -> String {
    headers
        .iter()
        .find(|value| value.starts_with(&format!("{name}=")))
        .cloned()
        .unwrap_or_else(|| panic!("missing Set-Cookie header for {name}: {headers:?}"))
}

#[tokio::test]
async fn test_logout_cookie_removal_localhost_attributes() {
    let (app, _) = common::create_test_app_with_frontend_url("http://localhost:5173").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header(
                    header::COOKIE,
                    "midpen_token=test; midpen_logged_in=1; midpen_oauth_nonce=nonce",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let set_cookies = set_cookie_headers(&response);
    let token_cookie = find_cookie(&set_cookies, "midpen_token");
    let hint_cookie = find_cookie(&set_cookies, "midpen_logged_in");
    let nonce_cookie = find_cookie(&set_cookies, "midpen_oauth_nonce");

    assert!(token_cookie.contains("Path=/"));
    assert!(token_cookie.contains("HttpOnly"));
    assert!(token_cookie.contains("SameSite=Lax"));
    assert!(token_cookie.contains("Max-Age=0"));
    assert!(!token_cookie.contains("Secure"));
    assert!(!token_cookie.contains("Domain="));

    assert!(hint_cookie.contains("Path=/"));
    assert!(hint_cookie.contains("SameSite=Lax"));
    assert!(hint_cookie.contains("Max-Age=0"));
    assert!(!hint_cookie.contains("HttpOnly"));
    assert!(!hint_cookie.contains("Secure"));
    assert!(!hint_cookie.contains("Domain="));

    assert!(nonce_cookie.contains("Path=/auth/strava/callback"));
    assert!(nonce_cookie.contains("HttpOnly"));
    assert!(nonce_cookie.contains("SameSite=Lax"));
    assert!(nonce_cookie.contains("Max-Age=0"));
    assert!(!nonce_cookie.contains("Secure"));
    assert!(!nonce_cookie.contains("Domain="));
}

#[tokio::test]
async fn test_logout_cookie_removal_production_domain_attributes() {
    let (app, _) = common::create_test_app_with_frontend_url("https://midpen-tracker.rolandd.dev").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header(
                    header::COOKIE,
                    "midpen_token=test; midpen_logged_in=1; midpen_oauth_nonce=nonce",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let set_cookies = set_cookie_headers(&response);
    let token_cookie = find_cookie(&set_cookies, "midpen_token");
    let hint_cookie = find_cookie(&set_cookies, "midpen_logged_in");
    let nonce_cookie = find_cookie(&set_cookies, "midpen_oauth_nonce");

    assert!(token_cookie.contains("Path=/"));
    assert!(token_cookie.contains("HttpOnly"));
    assert!(token_cookie.contains("SameSite=Lax"));
    assert!(token_cookie.contains("Max-Age=0"));
    assert!(token_cookie.contains("Secure"));
    assert!(!token_cookie.contains("Domain="));

    assert!(hint_cookie.contains("Path=/"));
    assert!(hint_cookie.contains("SameSite=Lax"));
    assert!(hint_cookie.contains("Max-Age=0"));
    assert!(hint_cookie.contains("Secure"));
    assert!(
        hint_cookie.contains("Domain=.rolandd.dev") || hint_cookie.contains("Domain=rolandd.dev")
    );
    assert!(!hint_cookie.contains("HttpOnly"));

    assert!(nonce_cookie.contains("Path=/auth/strava/callback"));
    assert!(nonce_cookie.contains("HttpOnly"));
    assert!(nonce_cookie.contains("SameSite=Lax"));
    assert!(nonce_cookie.contains("Max-Age=0"));
    assert!(nonce_cookie.contains("Secure"));
    assert!(!nonce_cookie.contains("Domain="));
}
