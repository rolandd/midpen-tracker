# Midpen-Strava justfile
# Run with: just <recipe>

# Default recipe - show help
default:
    @just --list

# ─── Configuration ────────────────────────────────────────────

# GCP project ID (set in .env, override with: just --set project YOUR_PROJECT, or uses gcloud config)
project := env_var_or_default("GCP_PROJECT_ID", `gcloud config get-value project 2>/dev/null || echo ""`)
region := "us-west1"

# Extract backend URL from web/.env (single source of truth)
backend_url := `grep VITE_API_URL web/.env | cut -d= -f2`

# Extract Strava client ID from terraform.tfvars (single source of truth)
strava_client_id := `grep strava_client_id infra/terraform.tfvars | cut -d'"' -f2`

# ─── Development ──────────────────────────────────────────────

# Format / lint preserve downloader script
lint-python:
    uvx ruff check --fix scripts/midpen.py
    uvx ruff format scripts/midpen.py

# Download preserves GeoJSON
fetch-preserves:
    uv run scripts/midpen.py

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

# ─── TypeScript Bindings ──────────────────────────────────────

# Generate TypeScript bindings from Rust DTOs
generate-bindings:
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf bindings
    cargo test export_bindings --
    cp bindings/web/src/lib/generated/*.ts web/src/lib/generated/
    rm -rf bindings
    echo "✅ TypeScript bindings generated in web/src/lib/generated/"

# Check bindings are up-to-date (CI safety net)
check-bindings:
    #!/usr/bin/env bash
    set -euo pipefail
    rm -rf bindings
    cargo test export_bindings -- 2>/dev/null
    for f in bindings/web/src/lib/generated/*.ts; do
        name=$(basename "$f")
        if ! diff -q "$f" "web/src/lib/generated/$name" >/dev/null 2>&1; then
            echo "❌ Binding $name is stale! Run: just generate-bindings"
            exit 1
        fi
    done
    rm -rf bindings
    echo "✅ TypeScript bindings are up-to-date"

# Setup git hooks for automatic binding generation
setup-hooks:
    git config core.hooksPath .githooks
    @echo "✅ Git hooks configured"

# ─── Build ────────────────────────────────────────────────────

# Build release binary locally
build-release:
    cargo build --release

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

# Register webhook with Strava (fetches secrets from Secret Manager)
register-webhook:
    #!/usr/bin/env bash
    set -euo pipefail
    
    echo "Fetching secrets from Secret Manager..."
    CLIENT_SECRET=$(gcloud secrets versions access latest --secret=STRAVA_CLIENT_SECRET --project={{project}})
    VERIFY_TOKEN=$(gcloud secrets versions access latest --secret=WEBHOOK_VERIFY_TOKEN --project={{project}})
    
    echo "Registering webhook at {{backend_url}}/webhook"
    curl -X POST https://www.strava.com/api/v3/push_subscriptions \
        -d client_id={{strava_client_id}} \
        -d client_secret=$CLIENT_SECRET \
        -d callback_url={{backend_url}}/webhook \
        -d verify_token=$VERIFY_TOKEN
    echo ""

# Check webhook subscriptions and verify current backend is registered
check-webhooks:
    #!/usr/bin/env bash
    set -euo pipefail
    
    echo "Fetching secrets..."
    CLIENT_SECRET=$(gcloud secrets versions access latest --secret=STRAVA_CLIENT_SECRET --project={{project}})
    
    echo "Current backend: {{backend_url}}"
    echo ""
    echo "Fetching webhook subscriptions..."
    
    RESPONSE=$(curl -s "https://www.strava.com/api/v3/push_subscriptions?client_id={{strava_client_id}}&client_secret=$CLIENT_SECRET")
    
    # Pretty print webhooks using jq if available
    if command -v jq &> /dev/null; then
        echo "$RESPONSE" | jq -r '.[] | "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\nID: \(.id)\nCallback: \(.callback_url)\nCreated: \(.created_at)\n"'
    else
        echo "$RESPONSE"
    fi
    
    # Check if our backend is registered
    EXPECTED_URL="{{backend_url}}/webhook"
    if echo "$RESPONSE" | grep -q "$EXPECTED_URL"; then
        echo "✅ Webhook for $EXPECTED_URL is registered"
    else
        echo "⚠️  Webhook for $EXPECTED_URL is NOT registered"
        echo "Run: just register-webhook"
    fi

# List current webhook subscriptions (pretty-printed)
list-webhooks:
    #!/usr/bin/env bash
    set -euo pipefail
    
    CLIENT_SECRET=$(gcloud secrets versions access latest --secret=STRAVA_CLIENT_SECRET --project={{project}})
    
    if command -v jq &> /dev/null; then
        curl -s "https://www.strava.com/api/v3/push_subscriptions?client_id={{strava_client_id}}&client_secret=$CLIENT_SECRET" | jq .
    else
        curl -s "https://www.strava.com/api/v3/push_subscriptions?client_id={{strava_client_id}}&client_secret=$CLIENT_SECRET"
    fi
