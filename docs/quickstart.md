# Quick start

Relay is one application plus PostgreSQL. Start with webhook delivery, then add an agent mailbox only if you need pull-based work.

```text
Start Relay ──▶ Create API key ──▶ Send webhook or mailbox message ──▶ Inspect
```

## 1. Start

```bash
cp .env.example .env   # set PJ_ENCRYPTION_KEY (base64 of 32 random bytes)
docker compose up -d
```

Open http://localhost:8080 and sign in with `PJ_ADMIN_USERNAME` / `PJ_ADMIN_PASSWORD` (defaults: `admin` / the password in your `.env`).

## 2. Create a destination and key

In the UI: **Destinations → create** (any public HTTPS URL — webhook.site works for testing), then **API keys → create**. Relay encrypts the full key at rest so the owner can copy it again later.

## 3. Send your first event

```bash
curl -X POST "http://localhost:8080/v1/destinations/$DESTINATION_ID/events" \
  -H "Authorization: Bearer pj_relay_YOUR_KEY" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: first-event" \
  -H "X-Event-Type: hello" \
  -d '{"hello":"relay"}'
```

`202` means committed. Watch **Events** for `QUEUED → DELIVERED` with the receiver's response stored as evidence. See [API and signing](api.md) for verification on the receiver side.

## 4. Optional: pull instead of push

```bash
curl -X POST "http://localhost:8080/v1/mail/agent-tasks/messages" \
  -H "Authorization: Bearer pj_relay_YOUR_KEY" \
  -H "Content-Type: application/json" \
  -d '{"task":"summarize"}'

curl -X POST "http://localhost:8080/v1/mail/agent-tasks/claim" \
  -H "Authorization: Bearer pj_relay_YOUR_KEY" \
  -H "Content-Type: application/json" -d '{"limit":10}'
```

Claim returns `claim_token`s; finish each message with `ack` (or `nack` to requeue). Details in [Agent mailbox](mailbox.md).

## 5. Optional: connect an agent with MCP

Create a Relay API key, keep it in your shell environment, and point the client at Relay's built-in Streamable HTTP endpoint:

```bash
export PJ_RELAY_API_KEY='pj_relay_YOUR_KEY'

codex mcp add promptjang-relay \
  --url http://localhost:8080/mcp \
  --bearer-token-env-var PJ_RELAY_API_KEY
```

The client can call `mail_push`, `mail_claim`, `mail_ack`, `mail_nack`, and `mail_list`. Every mailbox tool takes an explicit `mailbox` name. Relay owns PostgreSQL; the client receives only the MCP URL and API key.

For another Streamable HTTP MCP client, configure:

```text
URL: http://localhost:8080/mcp
Authorization: Bearer pj_relay_YOUR_KEY
```

The old `promptjang-relay-mcp` stdio binary and its direct `DATABASE_URL` mode remain temporarily for compatibility. Do not use it for new installations.

## Where next

- [Configuration reference](configuration.md) — every environment variable
- [Operations](operations.md) — backups, upgrades, scaling
- [Security](security.md) — private networks, secret handling
