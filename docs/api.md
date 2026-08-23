# Relay API and signing

The stable API uses `/api/v1` for administration and `/v1/destinations/:id/events` for ingestion. v0.1 `/api/*` and `/e/:id` routes remain deprecated aliases through v1.0 and send a `Deprecation` response header.

## Routes

| Area | Routes |
|---|---|
| Session | `POST /api/v1/session` · `DELETE /api/v1/session` |
| Destinations | `GET|POST /api/v1/destinations` · `GET|PATCH|DELETE /api/v1/destinations/:id` |
| Secret rotation | `POST …/:id/signing-secret/rotate` · `DELETE …/:id/signing-secret/previous` |
| Test delivery | `POST …/:id/test` |
| API keys | `GET|POST /api/v1/keys` · `DELETE /api/v1/keys/:id` |
| Events | `GET /api/v1/events?cursor=&destination_id=&status=&event_type=&limit=` · `GET /api/v1/events/:id` · `POST /api/v1/events/:id/replay` |
| System | `GET /api/v1/system` |
| Ingestion | `POST /v1/destinations/:id/events` |

API keys are unrestricted when `destination_ids` is empty, or restricted to the listed destinations.

## Ingest

```bash
curl -X POST "http://localhost:8080/v1/destinations/$DEST_ID/events" \
  -H "Authorization: Bearer pj_relay_YOUR_KEY" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: order-1042" \
  -H "X-Event-Type: order.created" \
  -d '{"order_id":"1042"}'
```

Returns `202` after PostgreSQL commits the payload and `QUEUED` state.

## Idempotency

`Idempotency-Key` is optional and scoped to one destination. Same key + exact payload bytes returns the original event; different bytes return `409`. Retries and replay never change this record.

## Signature verification

```ts
const signed = `${timestamp}.${rawBody}`                       // X-PromptJang-Timestamp + exact request bytes
const expected = hmacSha256(signingSecret, signed).hex()        // lowercase hex
timingSafeEqual(expected, req.header("X-PromptJang-Signature")) // constant-time compare
reject(Math.abs(now() - timestamp * 1000) > replayWindow)
```

After rotation, Relay also sends `X-PromptJang-Previous-Signature` signed with the prior secret. Accept either during migration; remove the old secret once in-flight retries have drained.

## Retry and replay

Any non-2xx response or network failure schedules the next configured retry; a 2xx ends delivery. When the retry budget is exhausted the event becomes `EXPIRED`. Replay creates a new linked event with its own stable ID. Delivery is at-least-once — verify signatures and tolerate duplicates.
