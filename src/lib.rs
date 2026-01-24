// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@kernel.org>

//! Midpen-Strava: Track adventures through Midpen Open Space Preserves
//!
//! This crate provides the backend API for processing Strava activities
//! and detecting which Midpen preserves were visited.

pub mod config;
pub mod db;
pub mod error;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod services;

use config::Config;
use db::FirestoreDb;
use services::{PreserveService, TasksService};

/// Shared application state.
pub struct AppState {
    pub config: Config,
    pub db: FirestoreDb,
    pub preserve_service: PreserveService,
    pub tasks_service: TasksService,
}
