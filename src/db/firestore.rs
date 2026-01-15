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
use crate::models::{Activity, User};

/// Firestore database client.
///
/// When the `gcp` feature is enabled, this connects to Firestore.
/// Without the feature, it provides stub implementations for local dev.
#[derive(Clone)]
pub struct FirestoreDb {
    #[allow(dead_code)]
    project_id: String,
    #[cfg(feature = "gcp")]
    client: firestore::FirestoreDb,
}

impl FirestoreDb {
    /// Create a new Firestore client.
    ///
    /// In production, this connects to Firestore.
    /// For local development with emulator, set FIRESTORE_EMULATOR_HOST.
    #[cfg(feature = "gcp")]
    pub async fn new(project_id: &str) -> Result<Self, AppError> {
        let client = firestore::FirestoreDb::new(project_id)
            .await
            .map_err(|e| AppError::Database(format!("Failed to connect to Firestore: {}", e)))?;

        tracing::info!(project = project_id, "Connected to Firestore");

        Ok(Self {
            project_id: project_id.to_string(),
            client,
        })
    }

    /// Create a stub client for local development without GCP.
    #[cfg(not(feature = "gcp"))]
    pub async fn new(project_id: &str) -> Result<Self, AppError> {
        tracing::warn!(
            project = project_id,
            "Firestore GCP feature not enabled - using stub client"
        );
        Ok(Self {
            project_id: project_id.to_string(),
        })
    }

    // ─── User Operations ─────────────────────────────────────────

    /// Get a user by their Strava athlete ID.
    #[cfg(feature = "gcp")]
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

    #[cfg(not(feature = "gcp"))]
    pub async fn get_user(&self, athlete_id: u64) -> Result<Option<User>, AppError> {
        tracing::debug!(athlete_id, "Stub: get_user");
        Ok(None)
    }

    /// Create or update a user.
    #[cfg(feature = "gcp")]
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

    #[cfg(not(feature = "gcp"))]
    pub async fn upsert_user(&self, _user: &User) -> Result<(), AppError> {
        tracing::debug!("Stub: upsert_user");
        Ok(())
    }

    // ─── Token Operations ────────────────────────────────────────

    /// Get encrypted tokens for a user.
    #[cfg(feature = "gcp")]
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

    #[cfg(not(feature = "gcp"))]
    pub async fn get_tokens(&self, athlete_id: u64) -> Result<Option<UserTokens>, AppError> {
        tracing::debug!(athlete_id, "Stub: get_tokens");
        Ok(None)
    }

    /// Store encrypted tokens for a user.
    #[cfg(feature = "gcp")]
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

    #[cfg(not(feature = "gcp"))]
    pub async fn set_tokens(&self, athlete_id: u64, _tokens: &UserTokens) -> Result<(), AppError> {
        tracing::debug!(athlete_id, "Stub: set_tokens");
        Ok(())
    }

    /// Delete tokens (for deauthorization).
    #[cfg(feature = "gcp")]
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

    #[cfg(not(feature = "gcp"))]
    pub async fn delete_tokens(&self, athlete_id: u64) -> Result<(), AppError> {
        tracing::debug!(athlete_id, "Stub: delete_tokens");
        Ok(())
    }

    // ─── Activity Operations ─────────────────────────────────────

    /// Get an activity by Strava ID.
    #[cfg(feature = "gcp")]
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

    #[cfg(not(feature = "gcp"))]
    pub async fn get_activity(&self, activity_id: u64) -> Result<Option<Activity>, AppError> {
        tracing::debug!(activity_id, "Stub: get_activity");
        Ok(None)
    }

    /// Store a processed activity.
    #[cfg(feature = "gcp")]
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
    #[cfg(not(feature = "gcp"))]
    pub async fn set_activity(&self, _activity: &Activity) -> Result<(), AppError> {
        tracing::debug!("Stub: set_activity");
        Ok(())
    }

    /// Delete an activity and update user stats.
    #[cfg(feature = "gcp")]
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

    #[cfg(not(feature = "gcp"))]
    pub async fn delete_activity(
        &self,
        activity_id: u64,
        _athlete_id: u64,
    ) -> Result<(), AppError> {
        tracing::debug!(activity_id, "Stub: delete_activity");
        Ok(())
    }

    // ─── User Stats Operations ──────────────────────────────────

    /// Get user stats aggregate document.
    ///
    /// Stored in `user_stats` collection, keyed by athlete_id.
    #[cfg(feature = "gcp")]
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

    #[cfg(not(feature = "gcp"))]
    pub async fn get_user_stats(
        &self,
        athlete_id: u64,
    ) -> Result<Option<crate::models::UserStats>, AppError> {
        tracing::debug!(athlete_id, "Stub: get_user_stats");
        Ok(None)
    }

    /// Store user stats aggregate document.
    ///
    /// Stored in `user_stats` collection, keyed by athlete_id.
    #[cfg(feature = "gcp")]
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

    #[cfg(not(feature = "gcp"))]
    pub async fn set_user_stats(
        &self,
        athlete_id: u64,
        _stats: &crate::models::UserStats,
    ) -> Result<(), AppError> {
        tracing::debug!(athlete_id, "Stub: set_user_stats");
        Ok(())
    }
}
