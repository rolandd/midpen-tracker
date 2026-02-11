// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Cloud Tasks service for rate-limited async processing.
//!
//! This service creates Cloud Tasks for:
//! - Processing new activities from webhooks
//! - Backfilling historical activities after OAuth (with progressive pagination)
//!
//! Uses the official google-cloud-tasks-v2 SDK.

use crate::error::AppError;
use crate::error::Result;
use futures_util::{stream, StreamExt};
use google_cloud_tasks_v2::client::CloudTasks;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::OnceCell;

const MAX_CONCURRENT_TASKS: usize = 100;

/// Payload sent to the activity processing task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessActivityPayload {
    pub activity_id: u64,
    pub athlete_id: u64,
    pub source: String, // "webhook" or "backfill"
}

/// Payload for continuing backfill with pagination.
/// This allows us to spread Strava API calls over time rather than
/// hitting the API repeatedly at login.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContinueBackfillPayload {
    pub athlete_id: u64,
    pub next_page: u32,
    pub after_timestamp: i64, // Unix timestamp for "activities after this date"
}

/// Payload for user deletion task (GDPR compliance).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteUserPayload {
    pub athlete_id: u64,
    pub source: String, // "webhook" or "user_request"
}

/// Payload for activity deletion task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteActivityPayload {
    pub activity_id: u64,
    pub athlete_id: u64,
    pub source: String, // "webhook"
}

/// Cloud Tasks client wrapper.
pub struct TasksService {
    project_id: String,
    queue_name: String,
    queue_path: String,
    service_account_email: String,
    client: OnceCell<CloudTasks>,
    enqueue_success_total: AtomicU64,
    enqueue_failure_total: AtomicU64,
}

impl TasksService {
    pub fn new(project_id: &str, region: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            queue_name: crate::config::ACTIVITY_QUEUE_NAME.to_string(),
            queue_path: format!(
                "projects/{}/locations/{}/queues/{}",
                project_id,
                region,
                crate::config::ACTIVITY_QUEUE_NAME
            ),
            service_account_email: format!(
                "midpen-tracker-api@{}.iam.gserviceaccount.com",
                project_id
            ),
            client: OnceCell::new(),
            enqueue_success_total: AtomicU64::new(0),
            enqueue_failure_total: AtomicU64::new(0),
        }
    }

    async fn client(&self) -> Result<&CloudTasks> {
        self.client
            .get_or_try_init(|| async move {
                let started_at = Instant::now();
                let client = CloudTasks::builder().build().await.map_err(|e| {
                    AppError::Internal(anyhow::anyhow!("Cloud Tasks client error: {}", e))
                })?;

                tracing::info!(
                    project = %self.project_id,
                    queue = %self.queue_name,
                    init_latency_ms = started_at.elapsed().as_millis(),
                    "Cloud Tasks client initialized"
                );

                Ok(client)
            })
            .await
    }

    /// Queue a single activity for processing.
    pub async fn queue_activity(
        &self,
        service_url: &str,
        payload: ProcessActivityPayload,
    ) -> Result<()> {
        self.queue_task(service_url, "/tasks/process-activity", &payload)
            .await
    }

    /// Queue a continue-backfill task for the next page.
    pub async fn queue_continue_backfill(
        &self,
        service_url: &str,
        payload: ContinueBackfillPayload,
    ) -> Result<()> {
        self.queue_task(service_url, "/tasks/continue-backfill", &payload)
            .await
    }

    /// Queue a user deletion task (GDPR compliance).
    pub async fn queue_delete_user(
        &self,
        service_url: &str,
        payload: DeleteUserPayload,
    ) -> Result<()> {
        tracing::info!(
            athlete_id = payload.athlete_id,
            source = %payload.source,
            "Queuing user deletion task"
        );
        self.queue_task(service_url, "/tasks/delete-user", &payload)
            .await
    }

    /// Queue an activity deletion task.
    pub async fn queue_delete_activity(
        &self,
        service_url: &str,
        payload: DeleteActivityPayload,
    ) -> Result<()> {
        self.queue_task(service_url, "/tasks/delete-activity", &payload)
            .await
    }

    /// Update stats and log queueing success
    fn record_enqueue_success(&self, endpoint: &str, enqueue_started: &std::time::Instant) {
        let success_total = self.enqueue_success_total.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::debug!(
            endpoint,
            queue = %self.queue_name,
            enqueue_latency_ms = enqueue_started.elapsed().as_millis(),
            enqueue_success_total = success_total,
            "Cloud Tasks enqueue succeeded"
        );
    }

    /// Update stats and log a queueing error
    fn record_enqueue_failure<E: std::fmt::Debug>(
        &self,
        endpoint: &str,
        enqueue_started: &std::time::Instant,
        error: E,
        message: &str,
    ) {
        let failure_total = self
            .enqueue_failure_total
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        tracing::warn!(
            endpoint,
            queue = %self.queue_name,
            enqueue_latency_ms = enqueue_started.elapsed().as_millis(),
            enqueue_failure_total = failure_total,
            ?error,
            message
        );
    }

    /// Generic task queuing helper.
    async fn queue_task<T: Serialize>(
        &self,
        service_url: &str,
        endpoint: &str,
        payload: &T,
    ) -> Result<()> {
        use google_cloud_tasks_v2::model::{HttpRequest, OidcToken, Task};

        let enqueue_started = Instant::now();

        let body = serde_json::to_vec(payload)
            .map_err(|e| AppError::Internal(anyhow::anyhow!("JSON error: {}", e)))?;

        let http_request = HttpRequest::default()
            .set_url(format!("{}{}", service_url, endpoint))
            .set_http_method("POST")
            .set_body(axum::body::Bytes::from(body))
            .set_headers(std::collections::HashMap::from([(
                "Content-Type".to_string(),
                "application/json".to_string(),
            )]))
            .set_oidc_token(
                OidcToken::default()
                    .set_service_account_email(self.service_account_email.clone())
                    .set_audience(service_url.to_string()),
            );

        let task = Task::default().set_http_request(http_request);

        let client = self.client().await.inspect_err(|e| {
            self.record_enqueue_failure(
                endpoint,
                &enqueue_started,
                e,
                "Cloud Tasks enqueue failed during client initialization",
            );
        })?;

        let enqueue_result = client
            .create_task()
            .set_parent(self.queue_path.clone())
            .set_task(task)
            .send()
            .await;

        match enqueue_result {
            Ok(_) => {
                self.record_enqueue_success(endpoint, &enqueue_started);
                Ok(())
            }
            Err(e) => {
                self.record_enqueue_failure(
                    endpoint,
                    &enqueue_started,
                    &e,
                    "Cloud Tasks enqueue failed",
                );
                Err(AppError::Internal(anyhow::anyhow!(
                    "Cloud Tasks create error: {}",
                    e
                )))
            }
        }
    }

    /// Queue multiple activities for backfill.
    pub async fn queue_backfill(
        &self,
        service_url: &str,
        athlete_id: u64,
        activity_ids: Vec<u64>,
    ) -> Result<usize> {
        let count = activity_ids.len();
        let batch_success = Arc::new(AtomicU64::new(0));
        let batch_failure = Arc::new(AtomicU64::new(0));

        stream::iter(activity_ids)
            .for_each_concurrent(MAX_CONCURRENT_TASKS, |activity_id| {
                let batch_success = Arc::clone(&batch_success);
                let batch_failure = Arc::clone(&batch_failure);
                async move {
                    let payload = ProcessActivityPayload {
                        activity_id,
                        athlete_id,
                        source: "backfill".to_string(),
                    };

                    match self.queue_activity(service_url, payload).await {
                        Ok(_) => {
                            batch_success.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => {
                            tracing::warn!(
                                activity_id,
                                error = ?e,
                                "Failed to queue activity for backfill"
                            );
                            batch_failure.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            })
            .await;

        tracing::info!(
            athlete_id,
            requested = count,
            succeeded = batch_success.load(Ordering::Relaxed),
            failed = batch_failure.load(Ordering::Relaxed),
            "Queued activities for backfill"
        );
        Ok(count)
    }
}
