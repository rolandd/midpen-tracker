# Midpen-Strava justfile
# Run with: just <recipe>

# Default recipe - show help
default:
    @just --list

# ─── Configuration ────────────────────────────────────────────

# GCP project ID (set in .env or override with: just --set project YOUR_PROJECT)
project := env_var_or_default("GCP_PROJECT_ID", "")
region := "us-west1"

# ─── Development ──────────────────────────────────────────────

# Run the API server locally
dev-api:
    cargo run

# Run with hot reload (requires cargo-watch)
dev-watch:
    cargo watch -x run

# Run tests
test:
    cargo test

# Run clippy lints
lint:
    cargo clippy -- -D warnings

# Format code
fmt:
    cargo fmt

# Check all (compile, test, lint, format check)
check-all: lint
    cargo test
    cargo fmt -- --check
    @echo "✅ All checks passed"

# ─── Build ────────────────────────────────────────────────────

# Build release binary locally
build-release:
    cargo build --release --features gcp

# Build Docker image locally
build-docker:
    docker build -t midpen-strava-api .

# Build and push to Artifact Registry
build-push:
    docker build -t {{region}}-docker.pkg.dev/{{project}}/midpen-strava/api:latest .
    docker push {{region}}-docker.pkg.dev/{{project}}/midpen-strava/api:latest

# ─── Deploy ───────────────────────────────────────────────────

# Deploy API to Cloud Run
deploy-api:
    gcloud run deploy midpen-strava-api \
        --image={{region}}-docker.pkg.dev/{{project}}/midpen-strava/api:latest \
        --region={{region}} \
        --project={{project}}

# Full deploy (build + push + deploy)
deploy: build-push deploy-api

# ─── Terraform ────────────────────────────────────────────────

# Terraform binary (using tfswitch in ~/bin)
terraform := "~/bin/terraform"

# Initialize Terraform
tf-init:
    cd infra && {{terraform}} init

# Plan Terraform changes
tf-plan:
    cd infra && {{terraform}} plan

# Apply Terraform changes
tf-apply:
    cd infra && {{terraform}} apply

# Destroy all resources (DANGER)
tf-destroy:
    cd infra && {{terraform}} destroy

# ─── Secrets ──────────────────────────────────────────────────

# Set Strava client secret
set-secret-strava secret:
    echo -n "{{secret}}" | gcloud secrets versions add STRAVA_CLIENT_SECRET \
        --data-file=- --project={{project}}

# Set JWT signing key (generate with: openssl rand -base64 32)
set-secret-jwt secret:
    echo -n "{{secret}}" | gcloud secrets versions add JWT_SIGNING_KEY \
        --data-file=- --project={{project}}

# Set webhook verify token
set-secret-webhook secret:
    echo -n "{{secret}}" | gcloud secrets versions add WEBHOOK_VERIFY_TOKEN \
        --data-file=- --project={{project}}

# Generate and set a random JWT signing key
generate-jwt-key:
    #!/usr/bin/env bash
    key=$(openssl rand -base64 32)
    echo -n "$key" | gcloud secrets versions add JWT_SIGNING_KEY \
        --data-file=- --project={{project}}
    echo "✅ JWT signing key generated and stored"

# ─── Strava Webhook ───────────────────────────────────────────

# Register webhook with Strava (run after deploy)
register-webhook url verify_token:
    curl -X POST https://www.strava.com/api/v3/push_subscriptions \
        -d client_id=$STRAVA_CLIENT_ID \
        -d client_secret=$STRAVA_CLIENT_SECRET \
        -d callback_url={{url}}/webhook \
        -d verify_token={{verify_token}}

# List current webhook subscriptions
list-webhooks:
    curl "https://www.strava.com/api/v3/push_subscriptions?client_id=$STRAVA_CLIENT_ID&client_secret=$STRAVA_CLIENT_SECRET"
