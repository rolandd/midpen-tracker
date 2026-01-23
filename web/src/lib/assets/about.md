<script>
  import { SocialLink } from '$lib/components';
</script>

An independent, <a href="https://github.com/rolandd/midpen-strava" target="_blank" rel="noopener" style="display:inline-flex;align-items:center;gap:4px;color:var(--color-primary);text-decoration:underline;text-underline-offset:2px;">open source <svg height="14" viewBox="0 0 16 16" width="14" fill="currentColor" style="display:inline-block;vertical-align:text-bottom;"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"></path></svg></a> tool built to help you explore the [Midpeninsula Regional Open Space District](https://www.openspace.org). Connect your Strava account to automatically track your progress across all preserves.

**Disclaimer**: This project is a personal hobby project created by Roland. It is **not** affiliated with, endorsed by, or connected to the Midpeninsula Regional Open Space District (MROSD) or Strava, Inc.

Inspired by Judy Cooks's story in [One Year, 25 Preserves](https://www.openspace.org/stories/one-year-25-preserves).

### How it works

<div class="steps">
  <div class="step">
    <span class="step-icon">🔌</span>
    <div class="step-text">
        <strong>Connect</strong>
        <span>Securely link your Strava account</span>
    </div>
  </div>
  <div class="step-line"></div>
  <div class="step">
    <span class="step-icon">🤖</span>
    <div class="step-text">
        <strong>Auto-detect</strong>
        <span>We check your activities against preserve boundaries</span>
    </div>
  </div>
  <div class="step-line"></div>
  <div class="step">
    <span class="step-icon">📍</span>
    <div class="step-text">
        <strong>Track</strong>
        <span>See your progress and catch 'em all</span>
    </div>
  </div>
</div>

### Privacy & Data

We take your privacy seriously. Here is exactly how we handle your data:

*   **Read**: We read your Strava profile (to identify you) and your activities (to check if they match preserve boundaries).
*   **Store**: We store your authentication tokens (encrypted), your activity metadata (e.g., date, device type, visit status), and the relevant activity IDs. We do **not** store your GPS tracks or maps.
*   **Modify**: We modify your Strava activity descriptions *only* to add preserve visit annotations (e.g., "🌲 Visited Rancho San Antonio").
*   **No Data Sharing**: We will never sell, rent, or voluntarily share your data with third parties for any purpose.
*   **Delete**: All data associated with you is permanently deleted if you disconnect the app in Strava or select "Delete my account" in this app.

### Credits & Data Sources

*   **Preserve Boundaries**: [MROSD Open Data Portal](https://opendata-mrosd.hub.arcgis.com/)
*   **Road Data**: [OpenStreetMap](https://www.openstreetmap.org) contributors
*   **Inspiration**: [MROSD Stories](https://www.openspace.org/stories)

### About Midpen

The [Midpeninsula Regional Open Space District](https://www.openspace.org) manages over 65,000 acres of land in 26 open space preserves. This app helps you discover and track visits to all of them.

