// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Midpen-Strava API Server
//!
//! Tracks adventures through Midpen Open Space Preserves by integrating
//! with Strava to detect which preserves were visited during activities.

use midpen_strava::{
    config::Config,
    db::FirestoreDb,
    services::{PreserveService, TasksService},
    AppState,
};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured JSON logging for GCP
    init_logging();

    // Load configuration from environment
    let config = Config::from_env().expect("Failed to load configuration");
    tracing::info!(port = config.port, "Starting Midpen-Strava API");

    // Initialize Firestore database
    let db = FirestoreDb::new(&config.gcp_project_id)
        .await
        .expect("Failed to connect to Firestore");

    // Load preserve boundaries
    let geo_path = "data/midpen_boundaries.geojson";
    tracing::info!(path = geo_path, "Loading preserve boundaries");
    let preserve_service =
        PreserveService::load_from_file(geo_path).expect("Failed to load preserve boundaries");
    tracing::info!(
        count = preserve_service.preserves().len(),
        "Preserve boundaries loaded"
    );

    // Initialize Cloud Tasks service
    let tasks_service = TasksService::new(&config.gcp_project_id);
    tracing::info!(
        project = %config.gcp_project_id,
        "Cloud Tasks service initialized"
    );

    // Build shared state
    let state = Arc::new(AppState {
        config: config.clone(),
        db,
        preserve_service,
        tasks_service,
    });

    // Build router
    let app = midpen_strava::routes::create_router(state);

    // Start server
    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(address = %addr, "Server listening");

    axum::serve(listener, app).await?;
    Ok(())
}

/// Initialize structured JSON logging (GCP-compliant).
fn init_logging() {
    let format = tracing_subscriber::fmt::layer()
        .json()
        .with_target(false)
        .with_current_span(true)
        .flatten_event(true);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("midpen_strava=debug".parse().unwrap())
                .add_directive("info".parse().unwrap()),
        )
        .with(format)
        .init();
}
