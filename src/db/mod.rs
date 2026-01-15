//! Database layer (Firestore).

pub mod firestore;

pub use firestore::FirestoreDb;

/// Collection names as constants.
pub mod collections {
    pub const USERS: &str = "users";
    pub const TOKENS: &str = "tokens";
    pub const ACTIVITIES: &str = "activities";
    pub const ACTIVITY_PRESERVES: &str = "activity_preserves";
    /// User stats aggregates (keyed by athlete_id)
    pub const USER_STATS: &str = "user_stats";
}
