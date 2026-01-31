# Service account for Cloud Run
resource "google_service_account" "cloudrun" {
  account_id   = "midpen-tracker-api"
  display_name = "Midpen-Tracker Cloud Run Service"
}

# IAM bindings - least privilege
resource "google_project_iam_member" "cloudrun_firestore" {
  project = var.project_id
  role    = "roles/datastore.user"
  member  = "serviceAccount:${google_service_account.cloudrun.email}"
}

resource "google_kms_crypto_key_iam_member" "cloudrun_kms" {
  crypto_key_id = google_kms_crypto_key.token_encryption.id
  role          = "roles/cloudkms.cryptoKeyEncrypterDecrypter"
  member        = "serviceAccount:${google_service_account.cloudrun.email}"
}

resource "google_secret_manager_secret_iam_member" "cloudrun_secrets" {
  for_each = {
    strava  = google_secret_manager_secret.strava_client_secret.id
    jwt     = google_secret_manager_secret.jwt_signing_key.id
    webhook = google_secret_manager_secret.webhook_verify_token.id
  }

  secret_id = each.value
  role      = "roles/secretmanager.secretAccessor"
  member    = "serviceAccount:${google_service_account.cloudrun.email}"
}

resource "google_project_iam_member" "cloudrun_tasks" {
  project = var.project_id
  role    = "roles/cloudtasks.enqueuer"
  member  = "serviceAccount:${google_service_account.cloudrun.email}"
}

# Cloud Run service (set deploy_cloudrun=true after building/pushing image)
resource "google_cloud_run_v2_service" "api" {
  count    = var.deploy_cloudrun ? 1 : 0
  name     = "midpen-tracker-api"
  location = var.region

  template {
    service_account = google_service_account.cloudrun.email

    scaling {
      min_instance_count = 0
      max_instance_count = 2
    }

    containers {
      image = "${var.region}-docker.pkg.dev/${var.project_id}/${var.service_name}/api:latest"

      ports {
        container_port = 8080
      }

      env {
        name  = "STRAVA_CLIENT_ID"
        value = var.strava_client_id
      }

      env {
        name  = "WEBHOOK_PATH_UUID"
        value = var.webhook_path_uuid
      }

      env {
        name  = "GCP_PROJECT_ID"
        value = var.project_id
      }

      env {
        name  = "GCP_REGION"
        value = var.region
      }

      env {
        name  = "FRONTEND_URL"
        value = var.frontend_url
      }

      env {
        name  = "API_URL"
        value = "https://${var.api_host}"
      }

      env {
        name = "STRAVA_CLIENT_SECRET"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.strava_client_secret.secret_id
            version = "latest"
          }
        }
      }

      env {
        name = "JWT_SIGNING_KEY"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.jwt_signing_key.secret_id
            version = "latest"
          }
        }
      }

      env {
        name = "WEBHOOK_VERIFY_TOKEN"
        value_source {
          secret_key_ref {
            secret  = google_secret_manager_secret.webhook_verify_token.secret_id
            version = "latest"
          }
        }
      }

      resources {
        limits = {
          cpu    = "1"
          memory = "512Mi"
        }
      }
    }
  }

  depends_on = [
    google_project_service.apis,
    google_secret_manager_secret.strava_client_secret,
    google_secret_manager_secret.jwt_signing_key,
    google_secret_manager_secret.webhook_verify_token,
  ]
}

# Allow unauthenticated access (public API)
resource "google_cloud_run_v2_service_iam_member" "public" {
  count    = var.deploy_cloudrun ? 1 : 0
  project  = var.project_id
  location = var.region
  name     = google_cloud_run_v2_service.api[0].name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

# Domain Mapping (Custom Domain)
resource "google_cloud_run_domain_mapping" "api" {
  count    = var.deploy_cloudrun && var.api_host != "" ? 1 : 0
  name     = var.api_host
  location = var.region

  metadata {
    namespace = var.project_id
  }

  spec {
    route_name = google_cloud_run_v2_service.api[0].name
  }
}

# Outputs
output "api_url" {
  value       = var.deploy_cloudrun ? (var.api_host != "" ? "https://${var.api_host}" : google_cloud_run_v2_service.api[0].uri) : "(not deployed yet)"
  description = "Cloud Run API URL"
}

output "artifact_registry" {
  value       = "${var.region}-docker.pkg.dev/${var.project_id}/midpen-tracker"
  description = "Artifact Registry path for Docker images"
}
