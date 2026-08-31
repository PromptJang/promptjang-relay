# Agent mailbox and MCP

A mailbox holds durable work until an agent asks for it. Relay stores messages but does not run, schedule, or wake the agent.

```text
Agent A or app ──push──▶ [UNREAD mailbox]
                              │ claim with lease
                              ▼
                       [CLAIMED by Agent B]
                         │ ack       │ nack or lease expiry
                         ▼           ▼
                  ACKNOWLEDGED    UNREAD again
```

Lifecycle: `UNREAD → CLAIMED → ACKNOWLEDGED`. A nack or expired lease returns the message to `UNREAD`, giving at-least-once processing.

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

## MCP endpoint

Relay serves Streamable HTTP MCP at `/mcp`. It exposes `mail_push`, `mail_claim`, `mail_ack`, `mail_nack`, and `mail_list` and requires an unrestricted Relay API key on every request.

```bash
export PJ_RELAY_API_KEY='pj_relay_YOUR_KEY'
codex mcp add promptjang-relay \
  --url http://localhost:8080/mcp \
  --bearer-token-env-var PJ_RELAY_API_KEY
```

Agents name the mailbox explicitly on each push, claim, acknowledgement, or nack. This makes one MCP connection usable for several agent inboxes without leaking `DATABASE_URL` outside Relay.

Browser-originated requests are accepted only when `Origin` matches `Host`. Non-browser clients normally omit `Origin`. Bearer tokens must be sent through the `Authorization` header, never a URL query parameter.

See [Remote MCP](remote-mcp.md) before exposing Relay through a private network, VPN, or public hostname.


## Agent Skill

Install the portable [PromptJang Agent Skill](https://github.com/PromptJang/promptjang-relay-skill) using the [Agent Skills](https://agentskills.io/home) format:

```bash
npx --yes skills add PromptJang/promptjang-relay-skill --skill promptjang -y
```

A release copy remains in `skills/promptjang`. The skill teaches agents when and how to send, claim, acknowledge, retry, and return mailbox results; it does not start polling or wake agents.

The skill is intentionally scoped to PromptJang Relay and Relay One. Both expose the same five mailbox tools, so the skill does not need product detection or a Cloud compatibility layer.
