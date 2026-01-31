## 2026-01-21 - Secure Cloud Tasks Handlers on Cloud Run
**Vulnerability:** Publicly accessible task handler endpoints allowed potential DoS or data corruption.
**Learning:** On Cloud Run, the `X-CloudTasks-QueueName` header is stripped from external requests but preserved for internal Cloud Tasks requests.
**Prevention:** Enforce the presence of this header in task handlers to ensure requests originate from Cloud Tasks without managing secrets.
