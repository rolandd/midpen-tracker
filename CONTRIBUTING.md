# Contributing to Midpen Tracker

Thank you for your interest in contributing to Midpen Tracker! We welcome contributions from the community to help improve this project.

## Project Overview

**Midpen Tracker** tracks outdoor adventures through Midpeninsula Regional Open Space District preserves by analyzing Strava activities. It detects which preserves were visited based on GPS polylines.

## Architecture

```
┌─────────────────────┐      ┌─────────────────────────┐
│   SvelteKit SPA     │      │    Rust/Axum Backend    │
│  (Cloudflare Pages) │◄────►│     (Cloud Run)         │
└─────────────────────┘      └───────────┬─────────────┘
                                         │
                    ┌────────────────────┼────────────────────┐
                    │                    │                    │
            ┌───────▼────────┐  ┌────────▼───────┐  ┌────────▼────────┐
            │   Firestore    │  │  Cloud Tasks   │  │   Strava API    │
            │   (User Data)  │  │ (Async Queue)  │  │   (OAuth/Data)  │
            └────────────────┘  └────────────────┘  └─────────────────┘
```

| Component | Technology | Notes |
|-----------|------------|-------|
| Backend | Rust + Axum | Main API server |
| Frontend | SvelteKit (SPA) | Static adapter, client-side routing |
| Database | Firestore | NoSQL document store |
| Hosting | Cloud Run + Cloudflare Pages | Backend + Frontend |
| Task Queue | Cloud Tasks | Async activity processing (handles Strava rate limits) |
| Infrastructure | Terraform | All in `infra/` directory |
| Task Runner | Just | See `justfile` for all commands |

## Directory Structure

```
src/
├── config.rs         # Environment/secret config loading
├── lib.rs            # AppState and module exports
├── main.rs           # Entry point
├── db/               # Firestore client and operations
├── middleware/       # Auth middleware (JWT validation)
├── models/           # Data structures (User, Activity, Preserve, Stats)
├── routes/           # HTTP handlers (api, auth, tasks, webhook)
└── services/         # Business logic (Strava API, preserve detection, tasks)

web/src/
├── lib/components/   # Reusable UI components
├── lib/generated/    # TypeScript types from Rust (ts-rs)
├── lib/api.ts        # Backend API client
└── routes/           # SvelteKit pages

tests/                # Integration tests (many require Firestore emulator)
scripts/              # Utility scripts (emulator, data fetching)
infra/                # Terraform configuration
```

## Key Flows

### Authentication
1. User clicks "Connect with Strava" → OAuth redirect
2. Backend exchanges code for tokens → stores in Firestore
3. Backend issues JWT cookie to frontend
4. JWT validated via middleware on protected routes (`/api/*`)

### Activity Processing
1. Strava webhook → `POST /webhook` receives event
2. Backend enqueues Cloud Task → `POST /tasks/process-activity`
3. Task handler fetches activity polyline from Strava
4. `PreserveService` detects which preserves intersect
5. Results stored in Firestore

## Getting Started

1.  **Fork & clone** the repository
2.  **Install dependencies**:
    - [Rust](https://www.rust-lang.org/tools/install)
    - [Node.js](https://nodejs.org/)
    - [Just](https://github.com/casey/just) command runner
    - [Terraform](https://www.terraform.io/) (optional, for deployment)

3.  **Copy environment config**:
    ```bash
    cp .env.example .env
    # Fill in your Strava API credentials
    ```

4.  **Run locally**:
    ```bash
    just dev-api          # Backend on :8080
    cd web && npm run dev # Frontend on :5173
    ```

## Development Workflow

### Common Tasks

| Task | Command |
|------|---------|
| Run backend | `just dev-api` |
| Run frontend | `cd web && npm run dev` |
| Run all checks | `just check-all` |
| Generate TS bindings | `just generate-bindings` |
| Deploy | `just deploy` |

### Testing

```bash
cargo test                              # Unit tests
./scripts/test-with-emulator.sh         # Integration tests (requires Firestore emulator)
cd web && npm run check && npm run lint # Frontend checks
```

> **Note**: Many tests in `tests/` require the Firestore emulator. They will fail without it.

### Code Style

- **Rust**: `cargo fmt` and `cargo clippy -- -D warnings`
- **Frontend**: `npm run format` and `npm run lint` in `web/`

### TypeScript Bindings

Rust DTOs use `ts-rs` to generate TypeScript types. After modifying models:
```bash
just generate-bindings
```

Enable the pre-commit hook for automatic regeneration:
```bash
just setup-hooks
```

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `STRAVA_CLIENT_ID` | Yes | Strava OAuth app ID |
| `STRAVA_CLIENT_SECRET` | Yes | Strava OAuth secret |
| `JWT_SIGNING_KEY` | Yes | 32+ bytes for JWT signing |
| `WEBHOOK_VERIFY_TOKEN` | Yes | Strava webhook verification |
| `GCP_PROJECT_ID` | Prod | Google Cloud project |
| `FRONTEND_URL` | No | Frontend origin for CORS |
| `API_URL` | No | Backend URL for Cloud Tasks callbacks |

## Submitting Changes

1.  Create a branch: `git checkout -b feature/my-feature`
2.  Make changes with [conventional commits](https://www.conventionalcommits.org/)
3.  Ensure `just check-all` passes
4.  Push and open a Pull Request

## License

By contributing, you agree that your contributions will be licensed under the MIT License, as defined in the [LICENSE](LICENSE) file.
