# ADR-001: Token Caching with Lazy Decryption

**Status:** Accepted  
**Date:** 2026-02-01  
**Authors:** Roland Dreier

## Context

The Midpen Tracker application uses Google Cloud KMS to encrypt/decrypt Strava OAuth tokens stored in Firestore. The original implementation had two inefficiencies:

1. **Gratuitous KMS calls**: Both access token AND refresh token were decrypted on every Strava API call, even though the refresh token is only needed when the access token is expired (~0.1% of calls).

2. **No caching**: During backfill operations (processing 100+ activities for a single user), the same token was decrypted from KMS repeatedly—once per activity.

This resulted in approximately 200 KMS calls to process 100 activities, when theoretically only 1-2 calls should be necessary.

## Decision

We will implement a three-part optimization:

### 1. Lazy Refresh Token Decryption

Only decrypt the refresh token when the access token is actually expired. This halves KMS calls in the common case.

### 2. In-Memory Token Cache

Cache decrypted access tokens in memory (per Cloud Run instance) with their expiry time. Cache is keyed by `athlete_id`. This reduces KMS calls from O(n) to O(1) for burst operations.

### 3. Per-User Refresh Mutex with Cross-Instance Retry

To handle concurrent token refresh:

**Within a single Cloud Run instance:**
- Use a per-user async mutex (`tokio::sync::Mutex`) to serialize refresh operations
- Second task waits for first to complete, then uses cached result

**Across multiple Cloud Run instances:**
- Accept that a race can occur (rare: token refresh happens once per 6 hours per user)
- If Strava returns `invalid_grant` during refresh, assume another instance won the race
- Re-read tokens from Firestore (which now has the winner's fresh tokens)
- Cache and return those tokens

```rust
match self.client.refresh_token(&refresh_token).await {
    Ok(new_tokens) => { /* store and cache */ },
    Err(e) if e.contains("invalid_grant") => {
        // Cross-instance race: another instance already refreshed
        // Fetch the winner's tokens from Firestore
        return self.fetch_and_cache_from_db(athlete_id).await;
    }
    Err(e) => return Err(e),
}
```

## Alternatives Considered

### Distributed Lock (Redis/Memorystore)
- **Pros:** Prevents cross-instance races entirely
- **Cons:** Adds new infrastructure dependency, cost, operational complexity
- **Verdict:** Overkill for current scale

### Firestore Optimistic Locking
- **Pros:** No new dependencies
- **Cons:** Doesn't prevent the Strava API race, only the DB write race; adds complexity
- **Verdict:** Insufficient and complex

### Single Cloud Run Instance
- **Pros:** Zero cross-instance races
- **Cons:** Limits throughput, higher latency, single point of failure
- **Verdict:** Too limiting

## Consequences

### Positive

- **~99% reduction in KMS calls** during backfill (from 200 to 1-2 per 100 activities)
- **Lower latency**: Cache hit = 0ms vs ~50-100ms KMS round-trip
- **Cost savings**: KMS pricing is $0.03/10K operations; adds up at scale
- **Rate limit protection**: Won't hit KMS quotas during burst operations

### Negative

- **Increased code complexity**: ~50 lines of additional code in `StravaService`
- **New dependency**: `dashmap` crate for concurrent HashMap
- **Memory usage**: ~100 bytes per cached user (negligible)
- **Decrypted tokens in memory**: If Cloud Run instance is compromised, cached tokens are exposed (but attacker would also have KMS credentials)

### Neutral

- **Cross-instance race**: Accepted as rare occurrence, handled gracefully with retry
- **Cache invalidation**: Handled naturally by token expiry; no manual invalidation needed

## Implementation Notes

- Cache and locks live in `AppState`, shared across all handlers via `Arc`
- Double-check pattern: After acquiring lock, re-check cache before doing work
- `DashMap` provides concurrent access without global lock contention

## References

- [Google Cloud KMS Pricing](https://cloud.google.com/kms/pricing)
- [OAuth 2.0 Token Refresh](https://datatracker.ietf.org/doc/html/rfc6749#section-6)
- [Strava OAuth Documentation](https://developers.strava.com/docs/authentication/)
