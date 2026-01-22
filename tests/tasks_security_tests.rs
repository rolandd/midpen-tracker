//! Security tests for Cloud Task handlers.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

/// Create a test app without GCP dependencies
async fn create_test_app() -> axum::Router {
    use midpen_strava::config::Config;
    use midpen_strava::db::FirestoreDb;
    use midpen_strava::routes::create_router;
    use midpen_strava::services::{PreserveService, TasksService};
    use midpen_strava::AppState;
    use std::sync::Arc;

    let config = Config::default();
    let db = FirestoreDb::new(&config.gcp_project_id).await.unwrap();
    let preserve_service = PreserveService::default();
    let tasks_service = TasksService::new(&config.gcp_project_id);

    let state = Arc::new(AppState {
        config,
        db,
        preserve_service,
        tasks_service,
    });

    create_router(state)
}

#[tokio::test]
async fn test_process_activity_no_header_forbidden() {
    let app = create_test_app().await;

    let payload = json!({
        "activity_id": 12345,
        "athlete_id": 67890,
        "source": "test"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/process-activity")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_process_activity_with_header_allowed() {
    let app = create_test_app().await;

    let payload = json!({
        "activity_id": 12345,
        "athlete_id": 67890,
        "source": "test"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/process-activity")
                .header("content-type", "application/json")
                .header("x-cloudtasks-queuename", "activity-processing") // Authorization header with correct queue
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should NOT be Forbidden.
    // It will likely be INTERNAL_SERVER_ERROR because Strava service fails in test env
    // or OK if it mocks out early.
    // The key is that it passed the security check.
    assert_ne!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_process_activity_wrong_queue_name_forbidden() {
    let app = create_test_app().await;

    let payload = json!({
        "activity_id": 12345,
        "athlete_id": 67890,
        "source": "test"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/process-activity")
                .header("content-type", "application/json")
                .header("x-cloudtasks-queuename", "wrong-queue") // Wrong queue name
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_continue_backfill_no_header_forbidden() {
    let app = create_test_app().await;

    let payload = json!({
        "athlete_id": 67890,
        "next_page": 2,
        "after_timestamp": 1234567890
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/tasks/continue-backfill")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_string(&payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
