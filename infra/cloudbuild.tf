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

# Connect to GitHub (2nd Gen)
#
# ⚠️  MANUAL SETUP REQUIRED ⚠️
# Terraform cannot fully automate the creation of the GitHub App connection because it requires
# an interactive OAuth flow in the browser.
#
# 1. Go to Google Cloud Console -> Cloud Build -> Repositories (2nd Gen).
# 2. Click "Create Host Connection".
# 3. Region: us-west1 (must match var.region).
# 4. Name: github-connection (must match below).
# 5. Provider: GitHub.
# 6. Authorize the connection and install the app on your repo.
# 7. IMPORT this resource into Terraform:
#    terraform import google_cloudbuildv2_connection.github projects/YOUR_PROJECT_ID/locations/us-west1/connections/github-connection
#
# Once imported, Terraform can manage the configuration.
resource "google_cloudbuildv2_connection" "github" {
  location = var.region
  name     = "github-connection"

  github_config {}

  lifecycle {
    ignore_changes = [github_config]
  }
}

# Link the repository
resource "google_cloudbuildv2_repository" "midpen_strava" {
  location          = var.region
  name              = "midpen-strava"
  parent_connection = google_cloudbuildv2_connection.github.name
  remote_uri        = "https://github.com/rolandd/midpen-strava.git"
}

# Trigger on push to main
resource "google_cloudbuild_trigger" "main_deploy" {
  name        = "midpen-strava-deploy"
  description = "Deploy to Cloud Run on push to main"
  location    = var.region

  service_account = google_service_account.cloudbuild.id

  repository_event_config {
    repository = google_cloudbuildv2_repository.midpen_strava.id
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
    _REGION    = var.region
    _REPO_NAME = var.service_name
  }

  depends_on = [
    google_project_service.apis,
    google_artifact_registry_repository.main
  ]
}
