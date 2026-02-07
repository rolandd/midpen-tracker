# Firestore Database (nam5 = US multi-region for free tier + durability)
resource "google_firestore_database" "main" {
  project     = var.project_id
  name        = "(default)"
  location_id = "nam5"		# US multi-region redundancy, has free tier
  type        = "FIRESTORE_NATIVE"

  depends_on = [google_project_service.apis]
}

# KMS Keyring for token encryption
resource "google_kms_key_ring" "main" {
  name     = "midpen-tracker"
  location = var.region

  depends_on = [google_project_service.apis]
}

resource "google_kms_crypto_key" "token_encryption" {
  name            = "token-encryption"
  key_ring        = google_kms_key_ring.main.id
  rotation_period = "7776000s" # 90 days

  purpose = "ENCRYPT_DECRYPT"
}

# Secret Manager secrets
resource "google_secret_manager_secret" "strava_client_secret" {
  secret_id = "STRAVA_CLIENT_SECRET"

  replication {
    auto {}
  }

  depends_on = [google_project_service.apis]
}

resource "google_secret_manager_secret" "jwt_signing_key" {
  secret_id = "JWT_SIGNING_KEY"

  replication {
    auto {}
  }

  depends_on = [google_project_service.apis]
}

resource "google_secret_manager_secret" "webhook_verify_token" {
  secret_id = "WEBHOOK_VERIFY_TOKEN"

  replication {
    auto {}
  }

  depends_on = [google_project_service.apis]
}

resource "google_secret_manager_secret" "webhook_path_uuid" {
  secret_id = "WEBHOOK_PATH_UUID"

  replication {
    auto {}
  }

  depends_on = [google_project_service.apis]
}

# Cloud Tasks queue for rate-limited processing
resource "google_cloud_tasks_queue" "activity_processing" {
  name     = "activity-processing"
  location = var.region

  rate_limits {
    max_dispatches_per_second = var.cloud_tasks_rate_limit
    max_concurrent_dispatches = 2
  }

  retry_config {
    max_attempts  = 100     # ~4 days of retries with 1h backoff
    min_backoff   = "10s"
    max_backoff   = "3600s" # 1 hour
    max_doublings = 4
  }

  depends_on = [google_project_service.apis]
}

# Artifact Registry for container images
resource "google_artifact_registry_repository" "main" {
  location      = var.region
  repository_id = var.service_name
  format        = "DOCKER"

  # Set to true to test the policy before actual deletion
  cleanup_policy_dry_run = false

  # Policy 1: Keep any tagged image (e.g. 'latest', 'production', 'v1.0')
  cleanup_policies {
    id     = "keep-tagged"
    action = "KEEP"
    condition {
      tag_state = "TAGGED"
    }
  }

  # Policy 2: Always keep the last 4 uploaded versions (safety buffer)
  cleanup_policies {
    id     = "keep-recent-buffer"
    action = "KEEP"
    most_recent_versions {
      keep_count = 4
    }
  }

  # Policy 3: Delete everything else older than 30 days
  cleanup_policies {
    id     = "delete-stale"
    action = "DELETE"
    condition {
      older_than = "2592000s" # 30 days
    }
  }

  depends_on = [google_project_service.apis]
}
