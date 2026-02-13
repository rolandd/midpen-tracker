// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Services module - business logic layer.

pub mod activity;
pub mod google_oidc;
pub mod kms;
pub mod preserve;
pub mod strava;
pub mod tasks;

pub use activity::ActivityProcessor;
pub use google_oidc::{GoogleOidcVerifier, OidcError, VerifiedTaskPrincipal};
pub use kms::KmsService;
pub use preserve::PreserveService;
pub use strava::{OAuthResult, StravaService};
pub use tasks::TasksService;
