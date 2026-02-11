// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Middleware modules (authentication, security, etc.).

pub mod auth;
pub mod security;
pub mod tasks_auth;

pub use auth::require_auth;
pub use tasks_auth::require_tasks_auth;
