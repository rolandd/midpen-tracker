# Cloud Build Service Account
resource "google_service_account" "cloudbuild" {
  account_id   = "midpen-strava-build"
  display_name = "Cloud Build Service Account"
}

# Permissions for Cloud Build SA
resource "google_project_iam_member" "build_log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.cloudbuild.email}"
}

resource "google_project_iam_member" "build_artifact_writer" {
  project = var.project_id
  role    = "roles/artifactregistry.writer"
  member  = "serviceAccount:${google_service_account.cloudbuild.email}"
}

resource "google_project_iam_member" "build_run_admin" {
  project = var.project_id
  role    = "roles/run.admin"
  member  = "serviceAccount:${google_service_account.cloudbuild.email}"
}

# Allow Cloud Build to act as the Cloud Run runtime SA (to deploy it)
resource "google_service_account_iam_member" "build_act_as_run" {
  service_account_id = google_service_account.cloudrun.name
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${google_service_account.cloudbuild.email}"
}

# Connect to GitHub
# Note: You must have installed the "Cloud Build" GitHub App on the repo manually first!
resource "google_cloudbuild_trigger" "main_deploy" {
  name        = "midpen-strava-deploy"
  description = "Deploy to Cloud Run on push to main"
  location    = var.region

  service_account = google_service_account.cloudbuild.id

  github {
    owner = "rolandd"
    name  = "midpen-strava"
    push {
      branch = "^main$"
    }
  }

  # Only trigger if changes match these patterns
  included_files = [
    "src/**",
    "infra/**",
    "Cargo.toml",
    "Cargo.lock",
    "Dockerfile",
    "cloudbuild.yaml",
    "justfile"
  ]

  filename = "cloudbuild.yaml"

  substitutions = {
    _REGION = var.region
  }

  depends_on = [
    google_project_service.apis,
    google_artifact_registry_repository.main
  ]
}
