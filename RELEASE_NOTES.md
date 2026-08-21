# PromptJang Relay v0.2.0

PromptJang Relay is the production-beta successor to PromptJang Webhooks OSS v0.1.0, which remains available as a technical preview.

## Highlights

- Independent Relay product and stable destination-oriented v1 API.
- Exact request-byte persistence and delivery, destination-scoped API keys, soft deletion, encrypted signing secrets, and safe secret rotation.
- Configurable payload, rate, retention, concurrency, timeout, retry, recovery, and response-evidence safeguards.
- Optional OTLP/HTTP traces, metrics, and logs behind `PJ_OTEL_ENABLED`; disabled means no SDK or exporter activity.
- New operational UI for destinations, events, API keys, system state, and guided setup.
- Automatic v0.1 PostgreSQL migration and deprecated API aliases retained through v1.x.
- Signed AMD64 and ARM64 container image with SBOM and provenance.

Review [the migration guide](docs/migration-v01-v02.md) before upgrading an existing installation.
