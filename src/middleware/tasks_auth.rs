// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Cloud Tasks authentication middleware.

use crate::services::google_oidc::OidcError;
use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use std::sync::Arc;

/// Require queue header + valid Cloud Tasks OIDC token for `/tasks/*` routes.
pub async fn require_tasks_auth(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let queue_name_header = request.headers().get("x-cloudtasks-queuename");
    let is_valid_queue = queue_name_header
        .and_then(|h| h.to_str().ok())
        .map(|name| name == crate::config::ACTIVITY_QUEUE_NAME)
        .unwrap_or(false);

    if !is_valid_queue {
        tracing::warn!(
            header = ?queue_name_header,
            "Blocked tasks request with invalid queue header"
        );
        return Err(StatusCode::FORBIDDEN);
    }

    let auth_header = request.headers().get(header::AUTHORIZATION);

    let principal = state
        .google_oidc_verifier
        .verify_cloud_tasks_token(auth_header)
        .await
        .map_err(|err| match err {
            OidcError::Forbidden(reason) => {
                tracing::warn!(reason = %reason, "Blocked tasks request: invalid OIDC token");
                StatusCode::FORBIDDEN
            }
            OidcError::Transient(reason) => {
                tracing::error!(reason = %reason, "Tasks OIDC verification transient failure");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        })?;

    tracing::debug!(
        email = %principal.email,
        subject = %principal.subject,
        audience = %principal.audience,
        "Cloud Tasks OIDC verification succeeded"
    );

    Ok(next.run(request).await)
}
