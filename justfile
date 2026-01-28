# SPDX-License-Identifier: MIT
# Copyright 2026 Roland Dreier <roland@rolandd.dev>

# Midpen-Tracker justfile
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

# Extract service name (midpen-tracker) from terraform.tfvars
service_name := `grep service_name infra/terraform.tfvars | cut -d'"' -f2`

# ─── Development ──────────────────────────────────────────────

# Format / lint preserve downloader script
lint-python:
    uvx ruff check --fix scripts/midpen.py
    uvx ruff format scripts/midpen.py

# Download preserves GeoJSON
fetch-preserves:
    uv run scripts/midpen.py

# Sync frontend config from Terraform
sync-frontend-config:
    #!/usr/bin/env bash
    set -euo pipefail
    # This requires terraform state to be initialized and applied
    URL=$(cd infra && {{terraform}} output -raw frontend_url 2>/dev/null || echo "https://midpen-strava.pages.dev")
    
    echo "Syncing frontend URL: $URL"
    
    # Create web/.env if it doesn't exist
    touch web/.env
    
    # Update or append PUBLIC_BASE_URL
    if grep -q "PUBLIC_BASE_URL=" web/.env; then
        # Use a different delimiter (#) for sed to handle slashes in URL
        sed -i "s#PUBLIC_BASE_URL=.*#PUBLIC_BASE_URL=$URL#" web/.env
    else
        echo "PUBLIC_BASE_URL=$URL" >> web/.env
    fi
    echo "✅ web/.env updated"

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
    TS_RS_EXPORT_DIR=. cargo test export_bindings --features binding-generation --
    # Run prettier on the generated files to match project style
    cd web && npx prettier --write src/lib/generated/*.ts
    echo "✅ TypeScript bindings generated and formatted in web/src/lib/generated/"

# Check bindings are up-to-date (CI safety net)
check-bindings:
    #!/usr/bin/env bash
    set -euo pipefail
    # For check, we want to ensure no diffs are created.
    # We can run the test and check git status for web/src/lib/generated/
    
    # Ideally we'd generate to a temp dir and diff, but for now let's rely on git
    TS_RS_EXPORT_DIR=. cargo test export_bindings --features binding-generation -- 2>/dev/null
    
    if ! git diff --quiet web/src/lib/generated/; then
        echo "❌ Bindings changed! Run: just generate-bindings"
        exit 1
    fi
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
    docker build -t {{region}}-docker.pkg.dev/{{project}}/{{service_name}}/api:latest .
    docker push {{region}}-docker.pkg.dev/{{project}}/{{service_name}}/api:latest

# ─── Deploy ───────────────────────────────────────────────────

# Deploy API to Cloud Run
deploy-api:
    gcloud run deploy midpen-strava-api \
        --image={{region}}-docker.pkg.dev/{{project}}/{{service_name}}/api:latest \
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

# Delete all webhook subscriptions
delete-webhook:
    #!/usr/bin/env bash
    set -euo pipefail
    
    if ! command -v jq &> /dev/null; then
        echo "❌ jq is required for this recipe"
        exit 1
    fi

    echo "Fetching secrets..."
    CLIENT_SECRET=$(gcloud secrets versions access latest --secret=STRAVA_CLIENT_SECRET --project={{project}})
    
    echo "Fetching subscriptions..."
    SUBS=$(curl -s "https://www.strava.com/api/v3/push_subscriptions?client_id={{strava_client_id}}&client_secret=$CLIENT_SECRET")
    
    # Check for empty list
    COUNT=$(echo "$SUBS" | jq 'if type=="array" then length else 0 end')
    
    if [ "$COUNT" -eq "0" ]; then
        echo "No active subscriptions found."
        exit 0
    fi
    
    echo "Found $COUNT subscription(s). Deleting..."
    
    # Get IDs and delete each
    for id in $(echo "$SUBS" | jq -r '.[].id'); do
        echo "Deleting subscription ID: $id"
        curl -s -X DELETE "https://www.strava.com/api/v3/push_subscriptions/$id?client_id={{strava_client_id}}&client_secret=$CLIENT_SECRET"
        echo ""
    done
    
    echo "✅ Webhooks deleted"
