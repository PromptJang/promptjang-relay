# PromptJang Relay

**Durable delivery for webhooks and agents, on your PostgreSQL.**

Relay stores each accepted item before delivery. It can push signed webhooks to services or hold messages for agents to pull through API or MCP.

No PromptJang Cloud account. No usage billing. One self-hosted service for one trusted team.

> v0.4.0 is the current production beta. v0.2.0 remains available for receivers that still use Relay's previous signing contract.

## How it works

```text
Webhook push

Your app ──event──▶ Relay + PostgreSQL ──signed delivery + retries──▶ Your service

Agent mailbox

Agent or app ──message──▶ Relay mailbox ◀──claim / acknowledge──▶ CLI agent
```

| Need | Use |
|---|---|
| Deliver an event to an HTTP service | Webhook destination |
| Leave durable work for an agent | Agent mailbox |
| Let a local agent use the mailbox | Optional MCP companion |

Relay does not run or wake agents. Your CLI, script, or scheduler decides when an agent checks its mailbox.

## Why Relay

- Returns `202` only after PostgreSQL commits the exact payload bytes and queued state.
- Retries failed webhook deliveries with a fixed, visible schedule.
- Signs webhooks with Standard Webhooks v1.
- Prevents duplicate acceptance with optional idempotency keys.
- Requeues mailbox claims when a consumer fails or its lease expires.
- Keeps delivery attempts, response evidence, replay history, and operational state in one UI.
- Runs without required telemetry or a hosted control plane.

## Five-minute start

```bash
cp .env.example .env
# Set the owner password, database password, and a base64 32-byte PJ_ENCRYPTION_KEY.
docker compose up --build
```

Open <http://localhost:8080>, create a destination and a `pj_relay_` API key, then send:

```bash
curl -X POST 'http://localhost:8080/v1/destinations/DESTINATION_ID/events' \
  -H 'Authorization: Bearer pj_relay_YOUR_KEY' \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: order-1042-created' \
  -H 'X-Event-Type: order.created' \
  -d '{"order_id":"1042"}'
```

A byte-identical duplicate returns the original event. Reusing the same idempotency key with different bytes returns `409`.

See the [quick start](docs/quickstart.md) for webhook, mailbox, and MCP setup.

## Delivery contract

- At-least-once delivery; receivers must tolerate duplicates.
- Exact accepted bytes signed as `event_id.timestamp.raw_body` with HMAC-SHA256.
- Standard `webhook-id`, `webhook-timestamp`, and `webhook-signature` headers.
- Optional `X-PromptJang-Event-Type` metadata.
- Retry delays: 60, 120, 240, 480, and 960 seconds by default.
- Interrupted processing recovers after five minutes by default.
- Redirects are not followed.
- Replay creates a linked event and preserves its source.
- Destination deletion is soft, so retained history remains inspectable.

There are no destination or API-key count caps. Payload, rate, retention, worker, timeout, retry, and private-network controls are operator-configured safeguards.

## Documentation

- [Quick start](docs/quickstart.md)
- [API, signing, and idempotency](docs/api.md)
- [Agent mailbox and MCP](docs/mailbox.md)
- [PromptJang Agent Skill](skills/promptjang/SKILL.md)
- [Configuration](docs/configuration.md)
- [OpenTelemetry](docs/observability.md)
- [Security](docs/security.md)
- [Operations](docs/operations.md)
- [v0.2 to v0.3 signing migration](docs/migration-v02-v03.md)

Telemetry is optional and disabled by default. When enabled, Relay exports traces, metrics, and logs through OTLP/HTTP while keeping delivery independent from collector availability.

## Development

```bash
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cd web && npm ci && npm run build
```

Apache-2.0 licensed. PromptJang Cloud is a separate product and is not required.
