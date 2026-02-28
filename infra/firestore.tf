// Supports preserve-scoped activity listing:
// WHERE athlete_id == ? AND preserve_name == ?
//   AND optional start_date > ?
// ORDER BY start_date DESC
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

// Supports user activity cursor pagination:
// WHERE athlete_id == ?
//   AND optional start_date > ?
//   AND optional cursor on (start_date, strava_activity_id)
// ORDER BY start_date DESC, strava_activity_id DESC
resource "google_firestore_index" "activities_by_date_and_id" {
  project    = var.project_id
  database   = "(default)"
  collection = "activities"

  fields {
    field_path = "athlete_id"
    order      = "ASCENDING"
  }

  fields {
    field_path = "start_date"
    order      = "DESCENDING"
  }

  fields {
    field_path = "strava_activity_id"
    order      = "DESCENDING"
  }
}
