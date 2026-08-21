# PromptJang Webhooks OSS

PromptJang Webhooks OSS is a small self-hosted webhook delivery product. It accepts JSON durably, signs outbound delivery, retries failures, keeps attempt history, and supports replay. The application is one Rust binary serving the API, Vue operational UI, and delivery worker.

`WEBHOOKS · SELF-HOSTED` is intentionally narrower than PromptJang Cloud. This repository has no organizations, Stripe, usage billing, agent mailboxes, MCP, A2A, Cloudflare storage, or hosted control-plane dependency.

## Run with Docker Compose

```bash
cp .env.example .env
# Replace every value in .env before starting.
docker compose up --build
```

Open [http://localhost:8080](http://localhost:8080). The first successful startup consumes `PJ_ADMIN_EMAIL` and `PJ_ADMIN_PASSWORD`, hashes the password with Argon2id, and creates the only owner. Later restarts ignore bootstrap credentials because the owner already exists.

Do not commit `.env`. For production, inject `PJ_ADMIN_PASSWORD`, the PostgreSQL password, and `DATABASE_URL` through your container platform's secret mechanism.

## Use an external PostgreSQL instance

The application depends only on `DATABASE_URL`. RDS, Cloud SQL, Neon, Supabase, or another compatible PostgreSQL instance can replace the example Compose database:

```bash
docker build -t promptjang-webhooks:0.1.0 .
docker run --rm -p 8080:8080 \
  -e DATABASE_URL='postgresql://USER:PASSWORD@HOST:5432/DATABASE?sslmode=require' \
  -e PJ_ADMIN_EMAIL='owner@example.com' \
  -e PJ_ADMIN_PASSWORD='use-a-container-secret-in-production' \
  promptjang-webhooks:0.1.0
```

SQLx migrations run automatically before the HTTP server starts. `/health` reports process liveness. `/ready` verifies PostgreSQL connectivity.

## Ingest a webhook event

Create an endpoint and a `pj_oss_` API key in the UI. Then:

```bash
curl -X POST 'http://localhost:8080/e/ENDPOINT_ID' \
  -H 'Authorization: Bearer pj_oss_YOUR_KEY' \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: order-1042-created' \
  -H 'X-Event-Type: order.created' \
  -d '{"order_id":"1042"}'
```

The server returns `202` only after PostgreSQL commits the payload and queued state. Reusing the same endpoint, idempotency key, and byte-identical payload returns the original event. A different payload returns `409`.

## Delivery contract

- HMAC-SHA256 over `timestamp.raw_body`
- Headers: `X-PromptJang-Signature`, `X-PromptJang-Timestamp`, and `X-PromptJang-Event-ID`
- Retry delays: 60, 120, 240, 480, and 960 seconds
- PostgreSQL queue claims use `FOR UPDATE SKIP LOCKED`
- Interrupted `PROCESSING` work is recovered after five minutes
- Redirects are not followed; non-2xx responses retry
- Replay creates a new event and never overwrites the source
- Limits: 10 endpoints, 5 API keys, 256 KB JSON payload, 1,000 accepted events per minute per endpoint

## Development

```bash
cargo test
cd web && npm install && npm run build
```

Apache-2.0 licensed. PromptJang Cloud is not required.
