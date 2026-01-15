# Allow Cloud Run SA to act as itself (needed for OIDC tokens in Cloud Tasks)
resource "google_service_account_iam_member" "cloudrun_oidc" {
  service_account_id = google_service_account.cloudrun.name
  role               = "roles/iam.serviceAccountUser"
  member             = "serviceAccount:${google_service_account.cloudrun.email}"
}
