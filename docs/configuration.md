# Relay configuration

| Variable | Default | Purpose |
|---|---:|---|
| `DATABASE_URL` | required | PostgreSQL connection string |
| `PJ_ENCRYPTION_KEY` | required | Base64-encoded 32-byte AES-GCM key |
| `PJ_ADMIN_USERNAME` | every start | Owner username (`PJ_ADMIN_EMAIL` accepted as a deprecated fallback) |
| `PJ_ADMIN_PASSWORD` | every start | Owner password; at least 12 characters (1 with `PJ_ALLOW_WEAK_PASSWORD=true`). Re-applied on startup when it differs, revoking existing sessions |
| `PJ_ALLOW_WEAK_PASSWORD` | `false` | Local development only: allow short passwords and owner re-bootstrap on every startup |
| `PJ_MAX_PAYLOAD_BYTES` | `1048576` | Accepted body maximum |
| `PJ_RATE_LIMIT_PER_DESTINATION_PER_MINUTE` | `10000` | Per-destination safeguard; `0` disables |
| `PJ_EVENT_RETENTION_DAYS` | `30` | Terminal event retention; `0` retains forever |
| `PJ_WORKER_CONCURRENCY` | `8` | Concurrent delivery loops |
| `PJ_DELIVERY_TIMEOUT_SECONDS` | `15` | Outbound HTTP timeout |
| `PJ_RETRY_DELAYS_SECONDS` | `60,120,240,480,960` | Retry schedule |
| `PJ_STUCK_AFTER_SECONDS` | `300` | Interrupted-processing recovery threshold |
| `PJ_RESPONSE_BODY_BYTES` | `10240` | Stored response evidence maximum |
| `PJ_DB_MAX_CONNECTIONS` | `20` | PostgreSQL pool maximum |
| `PJ_SESSION_TTL_SECONDS` | `86400` | Owner-session lifetime |
| `PJ_DESTINATION_ALLOW_PRIVATE_CIDRS` | empty | Comma-separated private CIDR allowlist |
| `PJ_ALLOW_INSECURE_HTTP` | `false` | Allow HTTP only to allowlisted private addresses |
| `PJ_EXTRA_CA_CERT_PATH` | empty | PEM CA bundle for private TLS |
| `PJ_OTEL_ENABLED` | `false` | Master OpenTelemetry gate |

When telemetry is enabled, set `OTEL_EXPORTER_OTLP_ENDPOINT`. Relay supports OTLP HTTP/protobuf and the standard variables `OTEL_SERVICE_NAME`, `OTEL_RESOURCE_ATTRIBUTES`, `OTEL_EXPORTER_OTLP_PROTOCOL`, `OTEL_EXPORTER_OTLP_HEADERS`, `OTEL_EXPORTER_OTLP_TIMEOUT`, `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, `OTEL_EXPORTER_OTLP_METRICS_ENDPOINT`, `OTEL_EXPORTER_OTLP_LOGS_ENDPOINT`, `OTEL_TRACES_EXPORTER`, `OTEL_METRICS_EXPORTER`, `OTEL_LOGS_EXPORTER`, `OTEL_PROPAGATORS`, and `OTEL_METRIC_EXPORT_INTERVAL`. `OTEL_SDK_DISABLED=true` always disables the SDK.
