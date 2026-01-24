# Midpen Tracker

Track your adventures through [Midpeninsula Regional Open Space
District](https://www.openspace.org/) preserves by analyzing your
Strava activities.

## Architecture

The application is built with a split frontend/backend architecture:

### Frontend
- **Framework**: [SvelteKit](https://kit.svelte.dev/) (SPA mode)
- **Styling**: Vanilla CSS with Design Tokens
- **Hosting**: [Cloudflare Pages](https://pages.cloudflare.com/)
- **Build**: Static site generation (`adapter-static`) with `index.html` fallback for client-side routing.

### Backend
- **Language**: Rust
- **Framework**: [Axum](https://github.com/tokio-rs/axum)
- **Hosting**: [Google Cloud Run](https://cloud.google.com/run) (Serverless container)
- **Database**: [Google Firestore](https://firebase.google.com/docs/firestore) (NoSQL)
- **Task Queue**: [Google Cloud Tasks](https://cloud.google.com/tasks) (Asynchronous activity processing)

### Authentication
1. User logs in via Strava OAuth2.
2. Backend exchanges Strava code for tokens.
3. Backend issues a custom JWT to the frontend.
4. Frontend uses JWT for subsequent API requests.

## Deployment

### Infrastructure
Infrastructure is managed via **Terraform** in the `infra/` directory. It provisions:
- Cloud Run Service
- Firestore Database
- Cloud Tasks Queues
- Artifact Registry
- Secret Manager secrets (Strava Client Secret, JWT Key, etc.)

### Backend Deployment
Managed via `justfile` recipes:
```bash
# Build Docker image, push to Artifact Registry, and deploy to Cloud Run
just deploy
```

### Frontend Deployment
The frontend is deployed to Cloudflare Pages automatically via git integration (or `wrangler` CLI).
- Build command: `npm run build`
- Output directory: `build`

## Development

- **Requirements**: Rust, Node.js, Just, gcloud CLI
- **Local Dev**:
  ```bash
  # Run backend locally
  just dev-api

  # Run frontend locally
  cd web && npm run dev
  ```
