# Security Policy

## Permissions Policy

The application (both backend API and frontend) enforces a strict **Permissions Policy** header to mitigate the risk of cross-site scripting (XSS) or other attacks abusing powerful browser features.

We adhere to the **Principle of Least Privilege**, disabling all features that are not explicitly required by the application.

### Implementation (Source of Truth)

The raw Permissions Policy string is defined in a single file: `permissions-policy.txt`.

This file is consumed by the build process to enforce consistency:
1. **Backend (Rust):** Loaded at compile time via `include_str!("../../permissions-policy.txt")`.
2. **Frontend (SvelteKit):** Injected at build time via `scripts/generate-security-config.js` into `web/functions/security-config.ts`.

### Rationale

| Feature | Status | Reason |
| :--- | :--- | :--- |
| `accelerometer` | Disabled `()` | Not used. Prevents device motion tracking fingerprinting. |
| `camera` | Disabled `()` | Not used. Prevents unauthorized camera access. |
| `geolocation` | Disabled `()` | Not used. Location data comes from Strava API, not the browser. |
| `gyroscope` | Disabled `()` | Not used. Prevents device orientation tracking fingerprinting. |
| `magnetometer` | Disabled `()` | Not used. Prevents device orientation tracking fingerprinting. |
| `microphone` | Disabled `()` | Not used. Prevents unauthorized audio recording. |
| `payment` | Disabled `()` | Not used. No payments processed. |
| `usb` | Disabled `()` | Not used. Prevents access to connected USB devices. |

### Updates

To update the policy:
1. Edit `permissions-policy.txt`.
2. Update this document (`SECURITY.md`) to reflect the rationale.
3. Rebuild both backend and frontend to apply changes.
