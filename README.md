# PromptJang Relay

**Reliable webhook delivery, on your PostgreSQL.**

PromptJang Relay is an independent Apache-2.0 product for a team sending application events to registered HTTP services. One Rust binary serves the API, operational Vue UI, and delivery workers. PostgreSQL stores accepted payloads, delivery state, attempts, and history.

Relay has no billing, organizations, mailboxes, MCP, A2A, Cloudflare dependency, hosted control plane, or required telemetry.

> v0.2.0 is a production beta. v0.1.0 remains available as a technical preview.

## Quick start

```bash
cp .env.example .env
# Set the owner, database password, and a base64-encoded 32-byte PJ_ENCRYPTION_KEY.
docker compose up --build
```

Generate an encryption key with your secret manager or an operating-system CSPRNG. Do not commit it. Open <http://localhost:8080>, create a destination, and create a destination-scoped `pj_relay_` API key.

```bash
curl -X POST 'http://localhost:8080/v1/destinations/DESTINATION_ID/events' \
  -H 'Authorization: Bearer pj_relay_YOUR_KEY' \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: order-1042-created' \
  -H 'X-Event-Type: order.created' \
  -d '{"order_id":"1042"}'
```

Relay returns `202` only after PostgreSQL commits the exact request bytes and queued state. A byte-identical idempotent duplicate returns the original event. Reusing the key with different bytes returns `409`.

## Delivery contract

- At-least-once delivery; receivers must tolerate duplicates.
- HMAC-SHA256 over `timestamp.raw_body` in `X-PromptJang-Signature`.
- Exact accepted bytes are delivered and signed.
- Default retry delays: 60, 120, 240, 480, and 960 seconds.
- Interrupted processing recovers after five minutes by default.
- Redirects are not followed.
- Replay creates a linked event and never overwrites its source.
- Destination deletion is soft; retained event history remains inspectable.

There are no endpoint or API-key count caps. Payload, rate, retention, worker capacity, timeouts, retries, and private-network access are operator-controlled safeguards.

## OpenTelemetry

Telemetry is disabled by default and initializes no SDK, exporter, background task, or network connection.

```env
PJ_OTEL_ENABLED=true
OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector:4318
OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf
OTEL_SERVICE_NAME=promptjang-relay
```

When enabled, Relay exports traces, metrics, and logs through OTLP/HTTP while continuing structured stdout logging. Collector failure never changes readiness or delivery behavior. See [the observability example](examples/observability/README.md).

## Documentation

- [API, signing, and idempotency](docs/api.md)
- [Configuration reference](docs/configuration.md)
- [OpenTelemetry and OTLP vendors](docs/observability.md)
- [Security and private networks](docs/security.md)
- [Operations: backup, upgrades, scaling, troubleshooting](docs/operations.md)
- [Upgrading from a pre-release build](docs/migration-v01-v02.md)

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cd web && npm ci && npm run build
```

Apache-2.0 licensed. PromptJang Cloud is not required and is a different product.
