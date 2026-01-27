# Codebase Audit Report: Midpen-Strava

**Date:** 2026-05-21
**Auditor:** Jules (AI Senior Full Stack Developer)
**Overall Rating:** ⭐⭐⭐⭐ (4/5) - **Excellent but Needs Polish**

## Executive Summary
The `midpen-strava` repository maintains a high standard of engineering. However, a verification pass reveals that **previously identified high-priority fixes have not yet been applied** to the codebase. Additionally, a deeper architectural review has uncovered potential scalability bottlenecks and data consistency issues.

## 🚨 Verification Findings (Regression/Missing Fixes)
The following items were reported as "addressed" but remain visible in the current codebase:

1.  **`Config` Safety (`src/config.rs`)**:
    *   **Status**: `impl Default` is still present with hardcoded test secrets.
    *   **Action**: Must be removed or converted to `test_default()`.
2.  **Incomplete API (`src/routes/api.rs`)**:
    *   **Status**: `get_activities` still contains a `TODO` and returns an empty list when filters are missing.
    *   **Action**: Implement full fetching or return `400 Bad Request`.
3.  **Error Handling (`src/routes/auth.rs`)**:
    *   **Status**: `unwrap()` on `SystemTime` is still present.

## New Prioritized Action Items

### 🟡 Medium (Scalability & Quality)
1.  **In-Memory Pagination** (`src/routes/api.rs`)
    *   **Issue**: `get_activities` fetches *all* activities for a preserve from the database before slicing them in memory (`results.into_iter()...`).
    *   **Impact**: Performance degradation as a user's activity history grows.
    *   **Recommendation**: Use Firestore's `offset` / `limit` or cursor-based pagination at the query level.
2.  **Date Format Inconsistency** (`src/services/activity.rs`)
    *   **Issue**: Activities use a custom `chrono_now_iso()` helper that returns **seconds as a string** (e.g., `"1735689600"`), whereas Users and Tokens use standard **RFC3339** (e.g., `"2026-05-21T10:00:00Z"`).
    *   **Impact**: Frontend date parsing complexity and potential sorting bugs.
    *   **Recommendation**: Standardize on `chrono::Utc::now().to_rfc3339()` everywhere.

### 🔵 Low (Cleanup)
1.  **`TODO` Cleanup**:
    *   `src/routes/api.rs`: "TODO: Query Firestore for user's activities"
    *   `src/db/firestore.rs`: "TODO: Could optimize by decrementing..."

## Open Source Readiness
*   **Ready for Showcase**: Yes.
*   **Ready for Contribution**: Needs the above fixes and a standard `CONTRIBUTING.md` before inviting community PRs.

## Rating Revision
*   **Previous**: 4.5/5 (Conditional on fixes)
*   **Current**: 4/5
*   *Reason*: The persistence of the "Default Config" safety risk and the discovery of in-memory pagination lowers the score slightly. Fixing these will bring it to a 5/5.
