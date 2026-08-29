# Agent mailbox (pull delivery)

Destinations push; the mailbox lets local tools and agents pull. Messages are stored per named mailbox with no delivery step: a producer pushes, a consumer claims with a lease, then acknowledges. At-least-once: a claim that is never acknowledged expires back to `UNREAD`.

Lifecycle: `UNREAD → CLAIMED → (ACKNOWLEDGED | back to UNREAD on nack or lease expiry)`.

## Endpoints

| Area | Routes | Auth |
|---|---|---|
| Push | `POST /v1/mail/:name/messages` | API key |
| Pull | `POST /v1/mail/:name/claim` | API key |
| Complete | `POST /v1/mail/:name/messages/:id/ack` · `POST /v1/mail/:name/messages/:id/nack` | API key |
| Inspect | `GET /api/v1/mail` · `GET /api/v1/mail/:name/messages?status=&limit=` · `DELETE /api/v1/mail/:name` | Owner session |

Mailbox names allow `a-z A-Z 0-9 - _ .` up to 100 characters. Push accepts the same `Idempotency-Key`, content-type, and exact-byte storage semantics as destination ingestion; a claim takes `{"limit": 1-100, "lease_seconds": 30-3600}` (defaults 10 and 300). `ack` requires the `claim_token` returned by the claim. Acknowledged messages follow the same retention window as terminal events.

## Example

```bash
# push (producer)
curl -X POST "http://localhost:8080/v1/mail/agent-tasks/messages" \
  -H "Authorization: Bearer pj_relay_YOUR_KEY" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: task-17" \
  -d '{"task":"summarize","target":"report.md"}'

# pull (consumer claims up to 10 messages for 5 minutes)
curl -X POST "http://localhost:8080/v1/mail/agent-tasks/claim" \
  -H "Authorization: Bearer pj_relay_YOUR_KEY" \
  -H "Content-Type: application/json" \
  -d '{"limit":10,"lease_seconds":300}'

# acknowledge one claimed message
curl -X POST "http://localhost:8080/v1/mail/agent-tasks/messages/$ID/ack" \
  -H "Authorization: Bearer pj_relay_YOUR_KEY" \
  -H "Content-Type: application/json" \
  -d '{"claim_token":"mlc_..."}'
```

`claim` re-queues messages whose lease expired before claiming new ones, so a crashed consumer loses nothing. Use `nack` to return a message immediately (for example, on a processing error). The claim response includes `payload` (exact bytes as UTF-8) and `payload_json` (parsed when valid JSON), plus the stored `traceparent` when telemetry accepted one.

## MCP server (local tool calling)

`promptjang-relay-mcp` is an MCP stdio server in the `mcp/` directory that exposes the mailbox as tools for local agents: `mail_push`, `mail_claim`, `mail_ack`, `mail_nack`, and `mail_list`. It reuses the same store layer and connects to the same PostgreSQL through `DATABASE_URL`.

```json
{
  "mcpServers": {
    "promptjang-relay": {
      "command": "promptjang-relay-mcp",
      "env": {
        "DATABASE_URL": "postgres://…",
        "RELAY_MAILBOX": "agent-tasks"
      }
    }
  }
}
```

`RELAY_MAILBOX` pins the default mailbox so agents can call `mail_push`/`mail_claim` without naming one; explicit `mailbox` arguments still win. Install from source with `cargo install --path mcp`.
