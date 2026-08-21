# Relay API and signing

The stable API uses `/api/v1` for administration and `/v1/destinations/:id/events` for ingestion. v0.1 `/api/*`, `/api/endpoints`, and `/e/:id` routes remain deprecated aliases through v1.0 and include a `Deprecation` response header.

## Routes

- `POST|DELETE /api/v1/session`
- `GET|POST /api/v1/destinations`
- `PATCH|DELETE /api/v1/destinations/:id`
- `POST /api/v1/destinations/:id/signing-secret/rotate`
- `DELETE /api/v1/destinations/:id/signing-secret/previous`
- `POST /api/v1/destinations/:id/test`
- `GET|POST /api/v1/keys`
- `DELETE /api/v1/keys/:id`
- `GET /api/v1/events?cursor=&destination_id=&status=&event_type=&limit=`
- `GET /api/v1/events/:id`
- `POST /api/v1/events/:id/replay`
- `GET /api/v1/system`
- `POST /v1/destinations/:id/events`

API keys are unrestricted when `destination_ids` is empty, or restricted to the listed destinations.

## Idempotency

`Idempotency-Key` is optional and scoped to one destination. The same key and exact payload bytes return the original event. Different bytes return `409`. Retries and replay do not change this record.

## Signature verification

Construct `timestamp + "." + raw_request_body`, compute HMAC-SHA256 using the destination signing secret, and compare the lowercase hexadecimal result to `X-PromptJang-Signature` using a constant-time comparison. Reject timestamps outside your chosen replay window.

After secret rotation, Relay also sends `X-PromptJang-Previous-Signature` using the prior secret. Receivers may accept either signature during migration, then remove the old secret after all retries signed during the transition have completed.

## Retry and replay

Any non-2xx response or network failure schedules the next configured retry. A successful 2xx response ends delivery. When the retry budget is exhausted the event becomes `EXPIRED`; replay creates a new linked event with its own stable ID. A receiver can see the same event ID more than once if it accepted a request but Relay stopped before recording the result.
