# Cloud Monitoring alert policy for errors
resource "google_monitoring_alert_policy" "api_errors" {
  count        = var.deploy_cloudrun ? 1 : 0
  display_name = "Midpen Tracker API Errors"
  combiner     = "OR"

  conditions {
    display_name = "Error log entries"

    condition_matched_log {
      filter = <<-EOT
        resource.type="cloud_run_revision"
        resource.labels.service_name="midpen-tracker-api"
        severity>=ERROR
      EOT

      label_extractors = {
        "error_message" = "EXTRACT(textPayload)"
      }
    }
  }

  notification_channels = []  # Add notification channels here

  alert_strategy {
    notification_rate_limit {
      period = "300s"  # 5 minutes between notifications
    }
    auto_close = "604800s"  # Auto-close after 7 days
  }

  documentation {
    content   = "API errors detected in Cloud Run logs. Check the logs at https://console.cloud.google.com/run/detail/${var.region}/midpen-tracker-api/logs"
    mime_type = "text/markdown"
  }
}

# Uptime check for API health (only when Cloud Run is deployed)
resource "google_monitoring_uptime_check_config" "api_health" {
  count        = var.deploy_cloudrun ? 1 : 0
  display_name = "Midpen Tracker API Health"
  timeout      = "10s"
  period       = "300s"  # Check every 5 minutes

  http_check {
    path         = "/health"
    port         = 443
    use_ssl      = true
    validate_ssl = true
  }

  monitored_resource {
    type = "uptime_url"
    labels = {
      project_id = var.project_id
      host       = trimprefix(trimsuffix(google_cloud_run_v2_service.api[0].uri, "/"), "https://")
    }
  }
}
