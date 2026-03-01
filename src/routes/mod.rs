// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! HTTP route handlers.

pub mod api;
pub mod auth;
pub mod tasks;
pub mod webhook;

use crate::middleware::auth::require_auth;
use crate::middleware::tasks_auth::require_tasks_auth;
use crate::AppState;
use axum::http::{header, Method};
use axum::{middleware, routing::get, Json, Router};
use serde::Serialize;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;
#[cfg(feature = "binding-generation")]
use ts_rs::TS;

#[derive(Serialize)]
#[cfg_attr(feature = "binding-generation", derive(TS))]
#[cfg_attr(
    feature = "binding-generation",
    ts(export, export_to = "web/src/lib/generated/")
)]
pub struct HealthResponse {
    pub status: String,
    pub build_id: String,
}

/// Health check response
async fn health_check() -> Json<HealthResponse> {
    let build_id = option_env!("BUILD_ID").unwrap_or("unknown").to_string();
    Json(HealthResponse {
        status: "ok".to_string(),
        build_id,
    })
}

/// Check if an origin is a localhost-like origin.
fn is_localhost(origin: &str) -> bool {
    if let Ok(uri) = origin.parse::<axum::http::Uri>() {
        uri.scheme_str() == Some("http")
            && matches!(
                uri.host(),
                Some("localhost") | Some("127.0.0.1") | Some("[::1]")
            )
    } else {
        false
    }
}

/// Build the complete router with all routes.
pub fn create_router(state: Arc<AppState>) -> Router {
    // CORS layer - allow requests from frontend URL, and allow localhost only in dev
    let frontend_url = state.config.frontend_url.clone();
    let is_dev = is_localhost(&frontend_url);

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            move |origin: &axum::http::HeaderValue, _request_parts: &axum::http::request::Parts| {
                let origin_str = origin.to_str().unwrap_or("");
                if origin_str == frontend_url {
                    return true;
                }

                is_dev && is_localhost(origin_str)
            },
        ))
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT]);

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(health_check))
        .merge(auth::routes())
        .merge(webhook::routes())
        .merge(tasks::routes().route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_tasks_auth,
        )));

    // Protected routes (auth required)
    let protected_routes =
        api::routes().route_layer(middleware::from_fn_with_state(state.clone(), require_auth));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .layer(middleware::from_fn(
            crate::middleware::security::add_security_headers,
        ))
        .layer(cors)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_localhost_logic() {
        assert!(is_localhost("http://localhost"));
        assert!(is_localhost("http://localhost:5173"));
        assert!(is_localhost("http://127.0.0.1"));
        assert!(is_localhost("http://127.0.0.1:5173"));
        assert!(is_localhost("http://[::1]"));
        assert!(is_localhost("http://[::1]:5173"));

        assert!(!is_localhost("https://localhost")); // Must be http
        assert!(!is_localhost("http://localhost.attacker.com"));
        assert!(!is_localhost("http://127.0.0.1.evil.com"));
        assert!(!is_localhost("http://example.com"));
    }
}
