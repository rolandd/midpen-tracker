terraform {
  required_version = ">= 1.0"

  required_providers {
    google = {
      source  = "hashicorp/google"
      version = "~> 5.0"
    }
    cloudflare = {
      source  = "cloudflare/cloudflare"
      version = "~> 4.0"
    }
  }

  # Remote state configuration (uncomment when bucket exists)
  # backend "gcs" {
  #   bucket = "midpen-strava-terraform-state"
  #   prefix = "terraform/state"
  # }
}
