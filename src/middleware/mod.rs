//! Authentication middleware for JWT validation.

pub mod auth;

pub use auth::require_auth;
