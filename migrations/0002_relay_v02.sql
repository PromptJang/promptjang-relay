ALTER TABLE endpoints RENAME TO destinations;
ALTER TABLE events RENAME COLUMN endpoint_id TO destination_id;

ALTER TABLE destinations
  ADD COLUMN deleted_at timestamptz,
  ADD COLUMN signing_secret_ciphertext bytea,
  ADD COLUMN previous_signing_secret_ciphertext bytea;

ALTER TABLE events
  ADD COLUMN payload_raw bytea,
  ADD COLUMN content_type text NOT NULL DEFAULT 'application/json',
  ADD COLUMN traceparent text,
  ADD COLUMN tracestate text;

UPDATE events SET payload_raw = convert_to(payload::text, 'UTF8') WHERE payload_raw IS NULL;
ALTER TABLE events ALTER COLUMN payload_raw SET NOT NULL;

ALTER INDEX events_idempotency_unique RENAME TO relay_events_idempotency_unique;
ALTER INDEX events_delivery_queue RENAME TO relay_events_delivery_queue;
CREATE INDEX relay_events_destination_created ON events(destination_id, created_at DESC);
CREATE INDEX relay_events_terminal_retention ON events(status, updated_at)
  WHERE status IN ('DELIVERED', 'EXPIRED');

CREATE TABLE api_key_destinations (
  api_key_id uuid NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
  destination_id uuid NOT NULL REFERENCES destinations(id) ON DELETE CASCADE,
  PRIMARY KEY (api_key_id, destination_id)
);

ALTER TABLE api_keys ADD COLUMN unrestricted boolean NOT NULL DEFAULT true;

CREATE TABLE login_attempts (
  email_hash text PRIMARY KEY,
  failed_count integer NOT NULL DEFAULT 0,
  window_started_at timestamptz NOT NULL DEFAULT now(),
  blocked_until timestamptz
);
