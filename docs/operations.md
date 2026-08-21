# Relay operations

`/health` is process liveness. `/ready` verifies PostgreSQL. OpenTelemetry collector availability is intentionally excluded from readiness.

Back up PostgreSQL using the tooling appropriate to your provider and back up `PJ_ENCRYPTION_KEY` through a separate secret-management path. Test restore into an isolated database before relying on it.

Monitor queue depth, queue delay, attempts, delivery duration, retries, expired events, stuck recovery, and cleanup through OTLP. Structured stdout logs remain available with telemetry disabled.

Before upgrades:

1. Back up PostgreSQL and the encryption key.
2. Read the release migration notes.
3. Run the new image against a restored staging copy.
4. Verify `/ready`, destination decryption, acceptance, delivery, history, and replay.

Multiple Relay instances can share PostgreSQL. Queue claims use `FOR UPDATE SKIP LOCKED`. Delivery remains at least once, including crashes after a receiver accepts a request but before Relay records success.

## Troubleshooting

- `/health` fails: the Relay process is unavailable.
- `/ready` fails: verify `DATABASE_URL`, network policy, TLS mode, credentials, connection capacity, and migrations.
- Events remain queued: inspect the System worker state, database connections, destination status, and structured logs.
- Repeated retries: inspect the event timeline, stored response evidence, DNS resolution, private CIDR allowlist, custom CA, and receiver signature verification.
- Telemetry export fails: inspect Collector reachability and OTLP headers. Delivery remains operational and stdout logs remain available.
