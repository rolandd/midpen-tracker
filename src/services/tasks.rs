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

/// Cloud Tasks client wrapper.
pub struct TasksService {
    project_id: String,
    location: String,
    queue_name: String,
}

impl TasksService {
    pub fn new(project_id: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            location: "us-west1".to_string(),
            queue_name: crate::config::ACTIVITY_QUEUE_NAME.to_string(),
        }
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
            .set_body(bytes::Bytes::from(body))
            .set_headers(std::collections::HashMap::from([(
                "Content-Type".to_string(),
                "application/json".to_string(),
            )]))
            .set_oidc_token(
                OidcToken::default()
                    .set_service_account_email(format!(
                        "midpen-strava-api@{}.iam.gserviceaccount.com",
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
    pub async fn queue_backfill(
        &self,
        service_url: &str,
        athlete_id: u64,
        activity_ids: Vec<u64>,
    ) -> Result<usize> {
        let count = activity_ids.len();

        stream::iter(activity_ids)
            .for_each_concurrent(MAX_CONCURRENT_TASKS, |activity_id| async move {
                let payload = ProcessActivityPayload {
                    activity_id,
                    athlete_id,
                    source: "backfill".to_string(),
                };

                if let Err(e) = self.queue_activity(service_url, payload).await {
                    tracing::warn!(activity_id, error = ?e, "Failed to queue activity for backfill");
                }
            })
            .await;

        tracing::info!(athlete_id, count, "Queued activities for backfill");
        Ok(count)
    }
}
