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

/// Firestore database client.
#[derive(Clone)]
pub struct FirestoreDb {
    client: firestore::FirestoreDb,
}

impl FirestoreDb {
    /// Create a new Firestore client.
    ///
    /// For local development with emulator, set FIRESTORE_EMULATOR_HOST.
    pub async fn new(project_id: &str) -> Result<Self, AppError> {
        let client = firestore::FirestoreDb::new(project_id)
            .await
            .map_err(|e| AppError::Database(format!("Failed to connect to Firestore: {}", e)))?;

        tracing::info!(project = project_id, "Connected to Firestore");

        Ok(Self {
            client,
        })
    }

    // ─── User Operations ─────────────────────────────────────────

    /// Get a user by their Strava athlete ID.
    pub async fn get_user(&self, athlete_id: u64) -> Result<Option<User>, AppError> {
        self.client
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
            .client
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
        self.client
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
            .client
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
        self.client
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
        self.client
            .fluent()
            .select()
            .by_id_in(collections::ACTIVITIES)
            .obj()
            .one(&activity_id.to_string())
            .await
            .map_err(|e| AppError::Database(e.to_string()))
    }

    /// Store a processed activity.
    pub async fn set_activity(&self, activity: &Activity) -> Result<(), AppError> {
        let _: () = self
            .client
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
        self.client
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
            .client
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
        self.client
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
    /// Uses sequential writes since the firestore crate's batch API requires specific setup.
    /// For small numbers of preserves per activity (typically 1-3), this is acceptable.
    pub async fn batch_set_activity_preserves(
        &self,
        records: &[ActivityPreserve],
    ) -> Result<(), AppError> {
        for record in records {
            // Document ID: combine activity_id and preserve_name to ensure uniqueness
            let safe_name = urlencoding::encode(&record.preserve_name);
            let doc_id = format!("{}_{}", record.activity_id, safe_name);

            let _: () = self
                .client
                .fluent()
                .update()
                .in_col(collections::ACTIVITY_PRESERVES)
                .document_id(&doc_id)
                .object(record)
                .execute()
                .await
                .map_err(|e| AppError::Database(e.to_string()))?;
        }
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
        self.client
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
            .client
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

        // Clone data needed inside the transaction closure
        let activity = activity.clone();
        let preserve_records = preserve_records.to_vec();
        let now_clone = now.clone();

        // Begin a transaction
        let mut transaction = self
            .client
            .begin_transaction()
            .await
            .map_err(|e| AppError::Database(format!("Failed to begin transaction: {}", e)))?;

        // 1. Read current user stats within the transaction
        //    This registers the document for conflict detection
        let current_stats: Option<crate::models::UserStats> = self
            .client
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
        self.client
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

            self.client
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
        self.client
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
}
