# Firestore Database (nam5 = US multi-region for free tier + durability)
resource "google_firestore_database" "main" {
  project     = var.project_id
  name        = "(default)"
  location_id = "nam5"
  type        = "FIRESTORE_NATIVE"

  depends_on = [google_project_service.apis]
}

# KMS Keyring for token encryption
resource "google_kms_key_ring" "main" {
  name     = "midpen-strava"
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

# Cloud Tasks queue for rate-limited processing
resource "google_cloud_tasks_queue" "activity_processing" {
  name     = "activity-processing"
  location = var.region

  rate_limits {
    max_dispatches_per_second = 0.1 # ~6 per minute (under Strava's 100/15min)
    max_concurrent_dispatches = 2
  }

  retry_config {
    max_attempts  = 5
    min_backoff   = "10s"
    max_backoff   = "300s"
    max_doublings = 4
  }

  depends_on = [google_project_service.apis]
}

# Artifact Registry for container images
resource "google_artifact_registry_repository" "main" {
  location      = var.region
  repository_id = "midpen-strava"
  format        = "DOCKER"

  depends_on = [google_project_service.apis]
}
