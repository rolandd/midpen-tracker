# Security Policy

## Permissions Policy

The application (both backend API and frontend) enforces a strict **Permissions Policy** header to mitigate the risk of cross-site scripting (XSS) or other attacks abusing powerful browser features.

We adhere to the **Principle of Least Privilege**, disabling all features that are not explicitly required by the application.

### Current Policy

The following policy string is the **Source of Truth** and must be used consistently in:
1. `src/middleware/security.rs` (Backend Rust Axum middleware)
2. `web/functions/_middleware.ts` (Frontend Cloudflare Pages middleware)

```text
accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()
```

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

When adding new features that require browser permissions:
1. Update this document first.
2. Update both backend and frontend implementations.
3. Verify consistency.
