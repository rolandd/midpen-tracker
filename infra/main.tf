# Midpen-Strava Infrastructure
#
# This Terraform configuration provisions:
# - Firestore database
# - KMS keyring and key for token encryption
# - Secret Manager secrets
# - Cloud Tasks queue for rate-limited processing
# - Artifact Registry for container images
# - Cloud Run service
# - IAM bindings with least privilege



variable "project_id" {
  description = "GCP Project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
  default     = "us-west1"
}

variable "strava_client_id" {
  description = "Strava OAuth Client ID"
  type        = string
}

variable "frontend_url" {
  description = "Public URL of the frontend (e.g., https://midpen-strava.pages.dev)"
  type        = string
}

variable "deploy_cloudrun" {
  description = "Whether to deploy Cloud Run (set true after pushing Docker image)"
  type        = bool
  default     = false
}

provider "google" {
  project = var.project_id
  region  = var.region
}

# Enable required APIs
resource "google_project_service" "apis" {
  for_each = toset([
    "run.googleapis.com",
    "cloudtasks.googleapis.com",
    "firestore.googleapis.com",
    "cloudkms.googleapis.com",
    "secretmanager.googleapis.com",
    "artifactregistry.googleapis.com",
    "cloudbuild.googleapis.com",
  ])

  service            = each.key
  disable_on_destroy = false
}

output "frontend_url" {
  value       = var.frontend_url
  description = "Public URL of the frontend"
}
