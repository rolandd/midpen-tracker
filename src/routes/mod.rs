//! HTTP route handlers.

pub mod api;
pub mod auth;
pub mod tasks;
pub mod webhook;

use crate::middleware::auth::require_auth;
use crate::AppState;
use axum::{middleware, routing::get, Json, Router};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tracing::Level;

/// Health check response
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
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
        .allow_methods(Any)
        .allow_headers(Any);

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
        .layer(cors)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .with_state(state)
}
