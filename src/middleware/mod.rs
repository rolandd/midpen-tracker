// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@kernel.org>

//! Authentication middleware for JWT validation.

pub mod auth;

pub use auth::require_auth;
