resource "google_firestore_index" "activity_preserves_by_date" {
  project    = var.project_id
  database   = "(default)"
  collection = "activity_preserves"

  fields {
    field_path = "athlete_id"
    order      = "ASCENDING"
  }

  fields {
    field_path = "preserve_name"
    order      = "ASCENDING"
  }

  fields {
    field_path = "start_date"
    order      = "DESCENDING"
  }
}
