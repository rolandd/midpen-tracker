// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! HTTP route handlers.

pub mod api;
pub mod auth;
pub mod tasks;
pub mod webhook;

use crate::middleware::auth::require_auth;
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

/// Build the complete router with all routes.
pub fn create_router(state: Arc<AppState>) -> Router {
    // CORS layer - allow requests from frontend URL and localhost (for dev)
    let frontend_url = state.config.frontend_url.clone();
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            move |origin: &axum::http::HeaderValue, _request_parts: &axum::http::request::Parts| {
                let origin_str = origin.to_str().unwrap_or("");
                origin_str == frontend_url
                    || origin_str.starts_with("http://localhost")
                    || origin_str.starts_with("http://127.0.0.1")
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
        .merge(tasks::routes()); // Task handler (called by Cloud Tasks)

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
