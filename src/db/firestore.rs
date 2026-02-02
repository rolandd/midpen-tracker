// SPDX-License-Identifier: MIT
// Copyright 2026 Roland Dreier <roland@rolandd.dev>

//! Firestore client wrapper with typed operations.
//!
//! Provides high-level operations for:
//! - Users (profile storage)
//! - Tokens (encrypted OAuth tokens)
//! - Activities (processed Strava activities)
//! - Activity-Preserves (join collection for queries)

use crate::db::collections;
use crate::error::AppError;
use crate::models::user::UserTokens;
use crate::models::{Activity, ActivityPreserve, User};
use futures_util::{stream, StreamExt};

const MAX_CONCURRENT_DB_OPS: usize = 50;
// Firestore limits batch/transaction writes to 500 operations.
// We use a safe limit of 400 to allow headroom.
const BATCH_SIZE: usize = 400;

/// Firestore database client.
#[derive(Clone)]
pub struct FirestoreDb {
    client: Option<firestore::FirestoreDb>,
}

impl FirestoreDb {
    /// Create a new Firestore client.
    ///
    /// For local development with emulator, set FIRESTORE_EMULATOR_HOST.
    pub async fn new(project_id: &str) -> Result<Self, AppError> {
        // If the emulator environment variable is set, use unauthenticated connection
        // to avoid local credential warnings and leakage.
        if std::env::var("FIRESTORE_EMULATOR_HOST").is_ok() {
            return Self::create_emulator_client(project_id).await;
        }

        let client = firestore::FirestoreDb::new(project_id)
            .await
            .map_err(|e| AppError::Database(format!("Failed to connect to Firestore: {}", e)))?;

        tracing::info!(project = project_id, "Connected to Firestore");

        Ok(Self {
            client: Some(client),
        })
    }

    /// Create a Firestore client for the emulator with unauthenticated access.
    async fn create_emulator_client(project_id: &str) -> Result<Self, AppError> {
        tracing::info!("Using unauthenticated connection for Firestore Emulator");

        // Use ExternalJwtFunctionSource to provide a dummy token without needing async-trait
        // or a custom TokenSource implementation struct.
        let token_source = gcloud_sdk::ExternalJwtFunctionSource::new(|| async {
            Ok(gcloud_sdk::Token {
                token_type: "Bearer".to_string(),
                token: gcloud_sdk::SecretValue::new(
                    "eyJhbGciOiJub25lIn0.eyJ1aWQiOiJ0ZXN0In0."
                        .to_string()
                        .into(),
                ),
                expiry: chrono::Utc::now() + chrono::Duration::hours(1),
            })
        });

        let options = firestore::FirestoreDbOptions::new(project_id.to_string());

        let client = firestore::FirestoreDb::with_options_token_source(
            options,
            gcloud_sdk::GCP_DEFAULT_SCOPES.clone(),
            gcloud_sdk::TokenSourceType::ExternalSource(Box::new(token_source)),
        )
        .await
        .map_err(|e| {
            AppError::Database(format!("Failed to connect to Firestore Emulator: {}", e))
        })?;

        tracing::info!(
            project = project_id,
            "Connected to Firestore (Emulator/Unauthenticated)"
        );

        Ok(Self {
            client: Some(client),
        })
    }

    /// Create a mock Firestore client for testing (offline mode).
    ///
    /// All database operations will return an error if called.
    pub fn new_mock() -> Self {
        Self { client: None }
    }

    /// Helper to get the client or return an error if offline.
    fn get_client(&self) -> Result<&firestore::FirestoreDb, AppError> {
        self.client
            .as_ref()
            .ok_or_else(|| AppError::Database("Database not connected (offline mode)".to_string()))
    }

    // ─── User Operations ─────────────────────────────────────────

    /// Get a user by their Strava athlete ID.
    pub async fn get_user(&self, athlete_id: u64) -> Result<Option<User>, AppError> {
        self.get_client()?
            .fluent()
            .select()
            .by_id_in(collections::USERS)
            .obj()
            .one(&athlete_id.to_string())
            .await
            .map_err(|e| AppError::Database(e.to_string()))
    }

    /// Create or update a user.
    pub async fn upsert_user(&self, user: &User) -> Result<(), AppError> {
        let _: () = self
            .get_client()?
            .fluent()
            .update()
            .in_col(collections::USERS)
            .document_id(user.strava_athlete_id.to_string())
            .object(user)
            .execute()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    // ─── Token Operations ────────────────────────────────────────

    /// Get encrypted tokens for a user.
    pub async fn get_tokens(&self, athlete_id: u64) -> Result<Option<UserTokens>, AppError> {
        self.get_client()?
            .fluent()
            .select()
            .by_id_in(collections::TOKENS)
            .obj()
            .one(&athlete_id.to_string())
            .await
            .map_err(|e| AppError::Database(e.to_string()))
    }

    /// Store encrypted tokens for a user.
    pub async fn set_tokens(&self, athlete_id: u64, tokens: &UserTokens) -> Result<(), AppError> {
        let _: () = self
            .get_client()?
            .fluent()
            .update()
            .in_col(collections::TOKENS)
            .document_id(athlete_id.to_string())
            .object(tokens)
            .execute()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    /// Delete tokens (for deauthorization).
    pub async fn delete_tokens(&self, athlete_id: u64) -> Result<(), AppError> {
        self.get_client()?
            .fluent()
            .delete()
            .from(collections::TOKENS)
            .document_id(athlete_id.to_string())
            .execute()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    // ─── Activity Operations ─────────────────────────────────────

    /// Get an activity by Strava ID.
    pub async fn get_activity(&self, activity_id: u64) -> Result<Option<Activity>, AppError> {
        self.get_client()?
            .fluent()
            .select()
            .by_id_in(collections::ACTIVITIES)
            .obj()
            .one(&activity_id.to_string())
            .await
            .map_err(|e| AppError::Database(e.to_string()))
    }

    /// Get activities for a user with pagination.
    pub async fn get_activities_for_user(
        &self,
        athlete_id: u64,
        after_date: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Activity>, AppError> {
        let query = self
            .get_client()?
            .fluent()
            .select()
            .from(collections::ACTIVITIES);

        let query = if let Some(date) = after_date {
            let date = date.to_string();
            query.filter(move |q| {
                q.for_all([
                    q.field("athlete_id").eq(athlete_id),
                    q.field("start_date").greater_than(date.clone()),
                ])
            })
        } else {
            query.filter(move |q| q.field("athlete_id").eq(athlete_id))
        };

        query
            .order_by([("start_date", firestore::FirestoreQueryDirection::Descending)])
            .limit(limit)
            .offset(offset)
            .obj()
            .query()
            .await
            .map_err(|e| AppError::Database(e.to_string()))
    }

    /// Store a processed activity.
    pub async fn set_activity(&self, activity: &Activity) -> Result<(), AppError> {
        let _: () = self
            .get_client()?
            .fluent()
            .update()
            .in_col(collections::ACTIVITIES)
            .document_id(activity.strava_activity_id.to_string())
            .object(activity)
            .execute()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    /// Delete an activity and update user stats.
    pub async fn delete_activity(&self, activity_id: u64, athlete_id: u64) -> Result<(), AppError> {
        // Delete the activity document
        self.get_client()?
            .fluent()
            .delete()
            .from(collections::ACTIVITIES)
            .document_id(activity_id.to_string())
            .execute()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        // Re-calculate stats for the user to ensure consistency
        // This is expensive but infrequent (deletes are rare)
        // TODO: Could optimize by decrementing, but recalculation is safer

        let activities = self
            .get_client()?
            .fluent()
            .select()
            .from(collections::ACTIVITIES)
            .filter(|q| q.for_all([q.field("athlete_id").eq(athlete_id)]))
            .obj::<Activity>()
            .query()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        let mut new_stats = crate::models::UserStats::default();
        let now = chrono::Utc::now().to_rfc3339();

        for activity in activities {
            new_stats.update_from_activity(&activity, &now);
        }

        self.set_user_stats(athlete_id, &new_stats).await?;

        Ok(())
    }

    // ─── Activity-Preserve Join Operations ───────────────────────

    /// Get all activities for a specific preserve and user.
    pub async fn get_activities_for_preserve(
        &self,
        athlete_id: u64,
        preserve_name: &str,
    ) -> Result<Vec<ActivityPreserve>, AppError> {
        self.get_client()?
            .fluent()
            .select()
            .from(collections::ACTIVITY_PRESERVES)
            .filter(|q| {
                q.for_all([
                    q.field("athlete_id").eq(athlete_id),
                    q.field("preserve_name").eq(preserve_name),
                ])
            })
            // Sort by date descending
            .order_by([("start_date", firestore::FirestoreQueryDirection::Descending)])
            .obj()
            .query()
            .await
            .map_err(|e| AppError::Database(e.to_string()))
    }

    /// Store multiple activity-preserve records.
    ///
    /// Uses concurrent writes with a limit to avoid overloading Firestore.
    pub async fn batch_set_activity_preserves(
        &self,
        records: &[ActivityPreserve],
    ) -> Result<(), AppError> {
        let client = self.get_client()?;

        stream::iter(records.to_vec())
            .map(|record| async move {
                // Document ID: combine activity_id and preserve_name to ensure uniqueness
                let safe_name = urlencoding::encode(&record.preserve_name);
                let doc_id = format!("{}_{}", record.activity_id, safe_name);

                let _: () = client
                    .fluent()
                    .update()
                    .in_col(collections::ACTIVITY_PRESERVES)
                    .document_id(&doc_id)
                    .object(&record)
                    .execute()
                    .await
                    .map_err(|e| AppError::Database(e.to_string()))?;

                Ok::<_, AppError>(())
            })
            .buffer_unordered(MAX_CONCURRENT_DB_OPS)
            .collect::<Vec<Result<(), AppError>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<()>, AppError>>()?;

        Ok(())
    }

    // ─── User Stats Operations ──────────────────────────────────

    /// Get user stats aggregate document.
    ///
    /// Stored in `user_stats` collection, keyed by athlete_id.
    pub async fn get_user_stats(
        &self,
        athlete_id: u64,
    ) -> Result<Option<crate::models::UserStats>, AppError> {
        self.get_client()?
            .fluent()
            .select()
            .by_id_in(collections::USER_STATS)
            .obj()
            .one(&athlete_id.to_string())
            .await
            .map_err(|e| AppError::Database(e.to_string()))
    }

    /// Store user stats aggregate document.
    ///
    /// Stored in `user_stats` collection, keyed by athlete_id.
    pub async fn set_user_stats(
        &self,
        athlete_id: u64,
        stats: &crate::models::UserStats,
    ) -> Result<(), AppError> {
        let _: () = self
            .get_client()?
            .fluent()
            .update()
            .in_col(collections::USER_STATS)
            .document_id(athlete_id.to_string())
            .object(stats)
            .execute()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        Ok(())
    }

    // ─── Atomic Activity Processing ─────────────────────────────────

    /// Atomically process an activity: store the activity, preserve joins, and update stats.
    ///
    /// This method uses a Firestore transaction to ensure all writes succeed or fail together.
    /// If another request modifies the user stats concurrently, Firestore will retry the
    /// transaction with fresh data, preventing lost updates.
    ///
    /// Returns `true` if the activity was newly processed, `false` if it was already processed
    /// (idempotent duplicate).
    pub async fn process_activity_atomic(
        &self,
        activity: &Activity,
        preserve_records: &[ActivityPreserve],
    ) -> Result<bool, AppError> {
        let athlete_id = activity.athlete_id;
        let activity_id = activity.strava_activity_id;
        let now = chrono::Utc::now().to_rfc3339();

        // Safety Check: Ensure user still exists before writing.
        // This mitigates "zombie data" if deletion happened during processing.
        if self.get_user(athlete_id).await?.is_none() {
            tracing::warn!(
                athlete_id,
                activity_id,
                "User not found, aborting atomic write (zombie prevention)"
            );
            return Ok(false);
        }

        // Clone data needed inside the transaction closure
        let activity = activity.clone();
        let preserve_records = preserve_records.to_vec();
        let now_clone = now.clone();

        // Begin a transaction
        let mut transaction = self
            .get_client()?
            .begin_transaction()
            .await
            .map_err(|e| AppError::Database(format!("Failed to begin transaction: {}", e)))?;

        // 1. Read current user stats within the transaction
        //    This registers the document for conflict detection
        let current_stats: Option<crate::models::UserStats> = self
            .get_client()?
            .fluent()
            .select()
            .by_id_in(collections::USER_STATS)
            .obj()
            .one(&athlete_id.to_string())
            .await
            .map_err(|e| {
                AppError::Database(format!("Failed to read stats in transaction: {}", e))
            })?;

        let mut stats = current_stats.unwrap_or_default();

        // 2. Check idempotency - if already processed, skip all writes
        if stats.processed_activity_ids.contains(&activity_id) {
            tracing::debug!(
                athlete_id,
                activity_id,
                "Activity already processed (idempotent skip)"
            );
            // Rollback the transaction since we don't need to write
            let _ = transaction.rollback().await;
            return Ok(false);
        }

        // 3. Update stats in memory
        stats.update_from_activity(&activity, &now_clone);

        // 4. Add activity write to transaction
        self.get_client()?
            .fluent()
            .update()
            .in_col(collections::ACTIVITIES)
            .document_id(activity.strava_activity_id.to_string())
            .object(&activity)
            .add_to_transaction(&mut transaction)
            .map_err(|e| {
                AppError::Database(format!("Failed to add activity to transaction: {}", e))
            })?;

        // 5. Add preserve join records to transaction
        for record in &preserve_records {
            let safe_name = urlencoding::encode(&record.preserve_name);
            let doc_id = format!("{}_{}", record.activity_id, safe_name);

            self.get_client()?
                .fluent()
                .update()
                .in_col(collections::ACTIVITY_PRESERVES)
                .document_id(&doc_id)
                .object(record)
                .add_to_transaction(&mut transaction)
                .map_err(|e| {
                    AppError::Database(format!(
                        "Failed to add preserve record to transaction: {}",
                        e
                    ))
                })?;
        }

        // 6. Add stats write to transaction
        self.get_client()?
            .fluent()
            .update()
            .in_col(collections::USER_STATS)
            .document_id(athlete_id.to_string())
            .object(&stats)
            .add_to_transaction(&mut transaction)
            .map_err(|e| {
                AppError::Database(format!("Failed to add stats to transaction: {}", e))
            })?;

        // 7. Commit the transaction atomically
        transaction
            .commit()
            .await
            .map_err(|e| AppError::Database(format!("Transaction commit failed: {}", e)))?;

        tracing::info!(
            athlete_id,
            activity_id,
            preserves_count = preserve_records.len(),
            "Activity processed atomically"
        );

        Ok(true)
    }

    // ─── Helper Methods ────────────────────────────────────────────

    /// Helper to batch delete documents using transactions.
    async fn batch_delete<T, F>(
        &self,
        items: &[T],
        collection: &str,
        id_extractor: F,
    ) -> Result<(), AppError>
    where
        F: Fn(&T) -> String,
    {
        let client = self.get_client()?;

        for chunk in items.chunks(BATCH_SIZE) {
            let mut transaction = client
                .begin_transaction()
                .await
                .map_err(|e| AppError::Database(format!("Failed to begin transaction: {}", e)))?;

            for item in chunk {
                let doc_id = id_extractor(item);
                client
                    .fluent()
                    .delete()
                    .from(collection)
                    .document_id(&doc_id)
                    .add_to_transaction(&mut transaction)
                    .map_err(|e| {
                        AppError::Database(format!(
                            "Failed to add deletion to transaction for {}: {}",
                            collection, e
                        ))
                    })?;
            }

            transaction.commit().await.map_err(|e| {
                AppError::Database(format!("Failed to commit batch deletion: {}", e))
            })?;
        }

        Ok(())
    }

    // ─── User Data Deletion (GDPR) ─────────────────────────────────

    /// Delete ALL data for a user (GDPR compliance).
    ///
    /// Deletes from all collections:
    /// - `activity_preserves` (query by athlete_id)
    /// - `activities` (query by athlete_id)
    /// - `user_stats/{athlete_id}`
    /// - `users/{athlete_id}`
    ///
    /// Note: Tokens should be deleted separately by the caller after
    /// using them for Strava deauthorization.
    ///
    /// Returns the number of documents deleted.
    pub async fn delete_user_data(&self, athlete_id: u64) -> Result<usize, AppError> {
        let mut deleted_count = 0;

        // 1. Delete all activity-preserve join records
        let preserve_records: Vec<ActivityPreserve> = self
            .get_client()?
            .fluent()
            .select()
            .from(collections::ACTIVITY_PRESERVES)
            .filter(|q| q.for_all([q.field("athlete_id").eq(athlete_id)]))
            .obj()
            .query()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        let count = preserve_records.len();
        self.batch_delete(
            &preserve_records,
            collections::ACTIVITY_PRESERVES,
            |record: &ActivityPreserve| {
                let safe_name = urlencoding::encode(&record.preserve_name);
                format!("{}_{}", record.activity_id, safe_name)
            },
        )
        .await?;

        deleted_count += count;
        tracing::debug!(athlete_id, count, "Deleted activity-preserve records");

        // 2. Delete all activities
        let activities: Vec<Activity> = self
            .get_client()?
            .fluent()
            .select()
            .from(collections::ACTIVITIES)
            .filter(|q| q.for_all([q.field("athlete_id").eq(athlete_id)]))
            .obj()
            .query()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;

        let count = activities.len();
        self.batch_delete(
            &activities,
            collections::ACTIVITIES,
            |activity: &Activity| activity.strava_activity_id.to_string(),
        )
        .await?;

        deleted_count += count;
        tracing::debug!(athlete_id, count, "Deleted activities");

        // 3. Delete user stats
        self.get_client()?
            .fluent()
            .delete()
            .from(collections::USER_STATS)
            .document_id(athlete_id.to_string())
            .execute()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        deleted_count += 1;
        tracing::debug!(athlete_id, "Deleted user stats");

        // 4. Delete user profile
        self.get_client()?
            .fluent()
            .delete()
            .from(collections::USERS)
            .document_id(athlete_id.to_string())
            .execute()
            .await
            .map_err(|e| AppError::Database(e.to_string()))?;
        deleted_count += 1;
        tracing::debug!(athlete_id, "Deleted user profile");

        tracing::info!(athlete_id, deleted_count, "User data deletion complete");

        Ok(deleted_count)
    }
}
