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
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

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

/// Result of a batch backfill queue operation.
///
/// Provides detailed information about which activities were successfully queued
/// and which failed, allowing callers to handle partial failures appropriately.
#[derive(Debug, Clone, Default)]
pub struct BackfillResult {
    /// Number of activities successfully queued.
    pub queued: u32,
    /// Number of activities that failed to queue.
    pub failed: u32,
    /// Activity IDs that failed to queue.
    pub failed_ids: Vec<u64>,
}

impl BackfillResult {
    /// Returns true if all activities were successfully queued.
    pub fn is_complete_success(&self) -> bool {
        self.failed == 0
    }

    /// Returns true if all activities failed to queue.
    pub fn is_complete_failure(&self) -> bool {
        self.queued == 0 && self.failed > 0
    }

    /// Returns true if some activities succeeded and some failed.
    pub fn is_partial_failure(&self) -> bool {
        self.queued > 0 && self.failed > 0
    }
}

/// Cloud Tasks client wrapper.
pub struct TasksService {
    project_id: String,
    location: String,
    queue_name: String,
    /// Mock: Activity IDs that should fail when queued (test builds only).
    #[cfg(test)]
    mock_fail_ids: std::sync::Mutex<std::collections::HashSet<u64>>,
}

impl TasksService {
    pub fn new(project_id: &str, region: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            location: region.to_string(),
            queue_name: crate::config::ACTIVITY_QUEUE_NAME.to_string(),
            #[cfg(test)]
            mock_fail_ids: std::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }

    /// Set activity IDs that should fail when queued (test builds only).
    ///
    /// This allows testing partial failure scenarios in backfill operations.
    #[cfg(test)]
    pub fn set_mock_fail_ids(&self, ids: impl IntoIterator<Item = u64>) {
        let mut guard = self.mock_fail_ids.lock().unwrap();
        guard.clear();
        guard.extend(ids);
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

    /// Generic task queuing helper.
    async fn queue_task<T: Serialize>(
        &self,
        service_url: &str,
        endpoint: &str,
        payload: &T,
    ) -> Result<()> {
        use google_cloud_tasks_v2::client::CloudTasks;
        use google_cloud_tasks_v2::model::{HttpRequest, OidcToken, Task};

        let client = CloudTasks::builder()
            .build()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Cloud Tasks client error: {}", e)))?;

        let queue_path = format!(
            "projects/{}/locations/{}/queues/{}",
            self.project_id, self.location, self.queue_name
        );

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
                    .set_service_account_email(format!(
                        "midpen-tracker-api@{}.iam.gserviceaccount.com",
                        self.project_id
                    ))
                    .set_audience(service_url.to_string()),
            );

        let task = Task::default().set_http_request(http_request);

        let _response = client
            .create_task()
            .set_parent(queue_path)
            .set_task(task)
            .send()
            .await
            .map_err(|e| AppError::Internal(anyhow::anyhow!("Cloud Tasks create error: {}", e)))?;

        Ok(())
    }

    /// Queue multiple activities for backfill.
    ///
    /// Returns a `BackfillResult` with details about which activities were
    /// successfully queued and which failed. Callers should use this to
    /// accurately update pending counters based on actual success count.
    pub async fn queue_backfill(
        &self,
        service_url: &str,
        athlete_id: u64,
        activity_ids: Vec<u64>,
    ) -> BackfillResult {
        let count = activity_ids.len();
        let batch_success = Arc::new(AtomicU64::new(0));
        let failed_ids = Arc::new(tokio::sync::Mutex::new(Vec::new()));

        stream::iter(activity_ids)
            .for_each_concurrent(MAX_CONCURRENT_TASKS, |activity_id| {
                let batch_success = Arc::clone(&batch_success);
                let failed_ids = Arc::clone(&failed_ids);
                async move {
                    // Check for mock failures in test builds
                    #[cfg(test)]
                    {
                        let should_fail = self.mock_fail_ids.lock().unwrap().contains(&activity_id);
                        if should_fail {
                            tracing::warn!(activity_id, "Mock failure for activity");
                            failed_ids.lock().await.push(activity_id);
                            return;
                        }
                    }

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
                            failed_ids.lock().await.push(activity_id);
                        }
                    }
                }
            })
            .await;

        let queued = batch_success.load(Ordering::Relaxed) as u32;
        let failed_ids = Arc::try_unwrap(failed_ids)
            .expect("All tasks completed, should have sole ownership")
            .into_inner();
        let failed = failed_ids.len() as u32;

        tracing::info!(
            athlete_id,
            requested = count,
            succeeded = queued,
            failed = failed,
            "Queued activities for backfill"
        );

        BackfillResult {
            queued,
            failed,
            failed_ids,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backfill_result_complete_success() {
        let result = BackfillResult {
            queued: 5,
            failed: 0,
            failed_ids: vec![],
        };

        assert!(result.is_complete_success());
        assert!(!result.is_complete_failure());
        assert!(!result.is_partial_failure());
    }

    #[test]
    fn backfill_result_complete_failure() {
        let result = BackfillResult {
            queued: 0,
            failed: 3,
            failed_ids: vec![100, 200, 300],
        };

        assert!(!result.is_complete_success());
        assert!(result.is_complete_failure());
        assert!(!result.is_partial_failure());
    }

    #[test]
    fn backfill_result_partial_failure() {
        let result = BackfillResult {
            queued: 2,
            failed: 1,
            failed_ids: vec![300],
        };

        assert!(!result.is_complete_success());
        assert!(!result.is_complete_failure());
        assert!(result.is_partial_failure());
    }

    #[test]
    fn backfill_result_empty_is_success() {
        let result = BackfillResult::default();

        assert!(result.is_complete_success());
        assert!(!result.is_complete_failure());
        assert!(!result.is_partial_failure());
    }

    #[test]
    fn backfill_result_failed_ids_match_failed_count() {
        let result = BackfillResult {
            queued: 7,
            failed: 3,
            failed_ids: vec![100, 200, 300],
        };

        assert_eq!(result.failed_ids.len() as u32, result.failed);
    }

    #[tokio::test]
    async fn queue_backfill_mock_partial_failure() {
        let service = TasksService::new("test-project", "us-central1");

        service.set_mock_fail_ids([200, 300]);

        let result = service
            .queue_backfill("http://localhost", 12345, vec![100, 200, 300, 400])
            .await;

        // All 4 will fail: 200 and 300 due to mock, 100 and 400 due to no Cloud Tasks client
        assert!(
            result.failed_ids.contains(&200),
            "200 should be in failed_ids"
        );
        assert!(
            result.failed_ids.contains(&300),
            "300 should be in failed_ids"
        );
        assert_eq!(result.failed, 4);
        assert_eq!(result.queued, 0);
    }

    #[tokio::test]
    async fn queue_backfill_mock_complete_failure() {
        let service = TasksService::new("test-project", "us-central1");

        service.set_mock_fail_ids([100, 200, 300]);

        let result = service
            .queue_backfill("http://localhost", 12345, vec![100, 200, 300])
            .await;

        assert!(result.is_complete_failure());
        assert_eq!(result.queued, 0);
        assert_eq!(result.failed, 3);
        assert!(result.failed_ids.contains(&100));
        assert!(result.failed_ids.contains(&200));
        assert!(result.failed_ids.contains(&300));
    }

    #[tokio::test]
    async fn queue_backfill_mock_empty_input() {
        let service = TasksService::new("test-project", "us-central1");

        service.set_mock_fail_ids([100, 200]);

        let result = service
            .queue_backfill("http://localhost", 12345, vec![])
            .await;

        assert!(result.is_complete_success());
        assert_eq!(result.queued, 0);
        assert_eq!(result.failed, 0);
        assert!(result.failed_ids.is_empty());
    }

    #[tokio::test]
    async fn queue_backfill_mock_clear_between_calls() {
        let service = TasksService::new("test-project", "us-central1");

        service.set_mock_fail_ids([100, 200]);
        service.set_mock_fail_ids([300]);

        let result = service
            .queue_backfill("http://localhost", 12345, vec![100, 200, 300])
            .await;

        assert!(result.failed_ids.contains(&300));
        assert_eq!(result.failed, 3);
    }

    #[test]
    fn set_mock_fail_ids_clears_previous() {
        let service = TasksService::new("test-project", "us-central1");

        service.set_mock_fail_ids([1, 2, 3]);
        service.set_mock_fail_ids([4, 5]);

        let guard = service.mock_fail_ids.lock().unwrap();
        assert!(!guard.contains(&1));
        assert!(!guard.contains(&2));
        assert!(!guard.contains(&3));
        assert!(guard.contains(&4));
        assert!(guard.contains(&5));
    }
}
