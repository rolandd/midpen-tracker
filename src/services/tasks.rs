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

/// Result of a bulk enqueue operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnqueueResult {
    /// Number of tasks successfully queued.
    pub queued: usize,
    /// Number of tasks failed to queue.
    pub failed: usize,
    /// IDs of activities that failed to queue.
    pub failed_ids: Vec<u64>,
}

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

    #[cfg(test)]
    mock_mode: bool,
    #[cfg(test)]
    mock_fail_ids: std::sync::Arc<std::sync::RwLock<std::collections::HashSet<u64>>>,
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
            #[cfg(test)]
            mock_mode: false,
            #[cfg(test)]
            mock_fail_ids: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashSet::new(),
            )),
        }
    }

    #[cfg(test)]
    pub fn new_mock() -> Self {
        Self {
            project_id: "mock-project".to_string(),
            queue_name: "mock-queue".to_string(),
            queue_path: "mock-queue-path".to_string(),
            service_account_email: "mock-email".to_string(),
            client: OnceCell::new(),
            enqueue_success_total: AtomicU64::new(0),
            enqueue_failure_total: AtomicU64::new(0),
            mock_mode: true,
            mock_fail_ids: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashSet::new(),
            )),
        }
    }

    #[cfg(test)]
    pub fn set_mock_fail_ids(&self, ids: Vec<u64>) {
        let mut fail_ids = self.mock_fail_ids.write().unwrap();
        fail_ids.clear();
        for id in ids {
            fail_ids.insert(id);
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
        #[cfg(test)]
        if self.mock_mode {
            // For backfill testing, we might inspect payload to simulate failure for specific ID
            // A cleaner way for ProcessActivityPayload:
            let body = serde_json::to_string(payload).unwrap_or_default();
            let fail_ids = self.mock_fail_ids.read().unwrap();
            for id in fail_ids.iter() {
                if body.contains(&format!("\"activity_id\":{}", id)) {
                     return Err(AppError::Internal(anyhow::anyhow!(
                        "Mock failure for activity {}",
                        id
                    )));
                }
            }
            return Ok(());
        }

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
    /// Returns detailed results about which activities were queued and which failed.
    pub async fn queue_backfill(
        &self,
        service_url: &str,
        athlete_id: u64,
        activity_ids: Vec<u64>,
    ) -> Result<EnqueueResult> {
        let count = activity_ids.len();
        let batch_success = Arc::new(AtomicU64::new(0));
        let batch_failure = Arc::new(AtomicU64::new(0));

        let results: Vec<std::result::Result<(), u64>> = stream::iter(activity_ids)
            .map(|activity_id| {
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
                            Ok(())
                        }
                        Err(e) => {
                            tracing::warn!(
                                activity_id,
                                error = ?e,
                                "Failed to queue activity for backfill"
                            );
                            batch_failure.fetch_add(1, Ordering::Relaxed);
                            Err(activity_id)
                        }
                    }
                }
            })
            .buffer_unordered(MAX_CONCURRENT_TASKS)
            .collect()
            .await;

        let mut queued = 0;
        let mut failed = 0;
        let mut failed_ids = Vec::new();

        for res in results {
            match res {
                Ok(_) => queued += 1,
                Err(id) => {
                    failed += 1;
                    failed_ids.push(id);
                }
            }
        }

        tracing::info!(
            athlete_id,
            requested = count,
            succeeded = batch_success.load(Ordering::Relaxed),
            failed = batch_failure.load(Ordering::Relaxed),
            "Queued activities for backfill"
        );

        Ok(EnqueueResult {
            queued,
            failed,
            failed_ids,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_queue_backfill_partial_failure() {
        let tasks_service = TasksService::new_mock();

        // Fail activity 2 and 4
        tasks_service.set_mock_fail_ids(vec![2, 4]);

        let activity_ids = vec![1, 2, 3, 4, 5];
        let result = tasks_service
            .queue_backfill("http://mock-service", 123, activity_ids)
            .await
            .unwrap();

        assert_eq!(result.queued, 3);
        assert_eq!(result.failed, 2);

        // Sorting might be needed as buffer_unordered doesn't guarantee order
        let mut failed_ids = result.failed_ids;
        failed_ids.sort();
        assert_eq!(failed_ids, vec![2, 4]);
    }

    #[tokio::test]
    async fn test_queue_backfill_all_success() {
        let tasks_service = TasksService::new_mock();
        let activity_ids = vec![1, 2, 3];
        let result = tasks_service
            .queue_backfill("http://mock-service", 123, activity_ids)
            .await
            .unwrap();

        assert_eq!(result.queued, 3);
        assert_eq!(result.failed, 0);
        assert!(result.failed_ids.is_empty());
    }

    #[tokio::test]
    async fn test_queue_backfill_all_failure() {
        let tasks_service = TasksService::new_mock();
        tasks_service.set_mock_fail_ids(vec![1, 2, 3]);

        let activity_ids = vec![1, 2, 3];
        let result = tasks_service
            .queue_backfill("http://mock-service", 123, activity_ids)
            .await
            .unwrap();

        assert_eq!(result.queued, 0);
        assert_eq!(result.failed, 3);
        let mut failed_ids = result.failed_ids;
        failed_ids.sort();
        assert_eq!(failed_ids, vec![1, 2, 3]);
    }
}
