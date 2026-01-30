# Cloudflare Pages Configuration
#
# This manages the Cloudflare Pages project configuration.
# Note: The initial project-to-GitHub connection must be done manually in the Cloudflare UI.
# After that, this terraform manages environment variables and deployment settings.

# Cloudflare account ID (get from Cloudflare dashboard)
variable "cloudflare_account_id" {
  description = "Cloudflare Account ID"
  type        = string
}

# API token for Cloudflare (should have Pages edit permissions)
variable "cloudflare_api_token" {
  description = "Cloudflare API Token with Pages edit permissions"
  type        = string
  sensitive   = true
}

# Zone ID for DNS records
variable "cloudflare_zone_id" {
  description = "Cloudflare Zone ID for the custom domain"
  type        = string
}

# Backend API URL for production
variable "production_api_url" {
  description = "Production API URL for VITE_API_URL"
  type        = string
  default     = "https://midpen-tracker-api-nynxios3na-uw.a.run.app"
}

# Preview/staging API URL (optional)
variable "preview_api_url" {
  description = "Preview API URL for VITE_API_URL in preview deployments"
  type        = string
  default     = "https://midpen-tracker-api-nynxios3na-uw.a.run.app" # Can change to staging URL
}

provider "cloudflare" {
  api_token = var.cloudflare_api_token
}

# Cloudflare Pages Project
# Note: This assumes the project already exists (created via UI with GitHub connection)
# Terraform is used to manage configuration, not create the initial project
resource "cloudflare_pages_project" "midpen_tracker_frontend" {
  account_id        = var.cloudflare_account_id
  name              = "midpen-tracker"
  production_branch = "main"

  # Build configuration
  build_config {
    build_command       = "npm run build"
    destination_dir     = "build"
    root_dir            = "web"
  }

  # Source (GitHub connection)
  source {
    type = "github"
    config {
      owner                         = "rolandd"
      repo_name                     = "midpen-tracker"
      production_branch             = "main"
      pr_comments_enabled           = true
      deployments_enabled           = true
      production_deployment_enabled = true
      preview_deployment_setting    = "none"
      preview_branch_includes       = ["*"]
      preview_branch_excludes       = []
    }
  }

  # Deployment configuration
  deployment_configs {
    production {
      environment_variables = {
        NODE_VERSION    = "24"
        NODE_ENV        = "development"
        PUBLIC_API_URL  = var.api_host != "" ? "https://${var.api_host}" : var.production_api_url
        PUBLIC_BASE_URL = var.frontend_url
      }

      compatibility_date = "2024-01-01"
    }

    preview {
      environment_variables = {
        NODE_VERSION    = "24"
        NODE_ENV        = "development"
        PUBLIC_API_URL  = var.api_host != "" ? "https://${var.api_host}" : var.preview_api_url
        PUBLIC_BASE_URL = var.frontend_url # Or a preview URL if we had one
      }

      compatibility_date = "2024-01-01"
    }
  }
}

locals {
  # Parse domain from frontend_url (strip protocol)
  frontend_domain = replace(replace(var.frontend_url, "https://", ""), "http://", "")
  # Check if custom domain (not pages.dev)
  is_custom_domain = !endswith(local.frontend_domain, ".pages.dev")
}

# Domain name for DNS records
variable "domain_name" {
  description = "Root domain name for DNS records (e.g. rolandd.dev)"
  type        = string
}

# Bind custom domain if configured
resource "cloudflare_pages_domain" "custom_domain" {
  count        = local.is_custom_domain ? 1 : 0
  account_id   = var.cloudflare_account_id
  project_name = cloudflare_pages_project.midpen_tracker_frontend.name
  domain       = local.frontend_domain
}

# Create DNS record explicitly
resource "cloudflare_record" "frontend_cname" {
  count   = local.is_custom_domain ? 1 : 0
  zone_id = var.cloudflare_zone_id
  # Strip the domain name + dot from the full frontend domain to get the subdomain
  name    = replace(local.frontend_domain, ".${var.domain_name}", "")
  content = "${cloudflare_pages_project.midpen_tracker_frontend.name}.pages.dev"
  type    = "CNAME"
  proxied = true
}

# Backend API DNS (CNAME to ghs.googlehosted.com for Cloud Run Custom Domain)
resource "cloudflare_record" "api_cname" {
  count   = var.api_host != "" ? 1 : 0
  zone_id = var.cloudflare_zone_id
  # Strip the domain name from the api host to get the subdomain
  name    = replace(var.api_host, ".${var.domain_name}", "")
  content = "ghs.googlehosted.com"
  type    = "CNAME"
  proxied = true
}

# NOTE: For Strava webhooks to work with the proxied API, you MUST manually DISABLE
# "Bot Fight Mode" in the Cloudflare Dashboard (Security -> Bots).
# This feature is not exposed in the Terraform provider for non-Enterprise plans,
# so it cannot be managed here. If enabled, it will block Strava's validation requests.

# Output the Pages URL
output "pages_url" {
  description = "Cloudflare Pages URL"
  value       = var.frontend_url
}

output "pages_project_name" {
  description = "Cloudflare Pages project name"
  value       = cloudflare_pages_project.midpen_tracker_frontend.name
}
