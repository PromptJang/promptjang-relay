CREATE TABLE owners (
  id uuid PRIMARY KEY,
  username text NOT NULL UNIQUE,
  password_hash text NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE sessions (
  id uuid PRIMARY KEY,
  owner_id uuid NOT NULL REFERENCES owners(id) ON DELETE CASCADE,
  token_hash text NOT NULL UNIQUE,
  expires_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE api_keys (
  id uuid PRIMARY KEY,
  name text NOT NULL,
  prefix text NOT NULL,
  secret_hash text NOT NULL UNIQUE,
  unrestricted boolean NOT NULL DEFAULT true,
  last_used_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE destinations (
  id uuid PRIMARY KEY,
  name text NOT NULL,
  url text NOT NULL,
  signing_secret text NOT NULL,
  signing_secret_ciphertext bytea,
  previous_signing_secret_ciphertext bytea,
  enabled boolean NOT NULL DEFAULT true,
  deleted_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE events (
  id uuid PRIMARY KEY,
  destination_id uuid NOT NULL REFERENCES destinations(id) ON DELETE CASCADE,
  status text NOT NULL CHECK (status IN ('QUEUED','PROCESSING','DELIVERED','RETRYING','EXPIRED')),
  event_type text,
  correlation_id text,
  payload jsonb NOT NULL,
  payload_raw bytea NOT NULL,
  content_type text NOT NULL DEFAULT 'application/json',
  payload_sha256 text NOT NULL,
  idempotency_key_hash text,
  traceparent text,
  tracestate text,
  retry_count integer NOT NULL DEFAULT 0,
  max_retries integer NOT NULL DEFAULT 5,
  is_replay boolean NOT NULL DEFAULT false,
  source_event_id uuid REFERENCES events(id) ON DELETE SET NULL,
  next_attempt_at timestamptz NOT NULL DEFAULT now(),
  last_error text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX relay_events_idempotency_unique ON events(destination_id, idempotency_key_hash)
  WHERE idempotency_key_hash IS NOT NULL AND is_replay = false;
CREATE INDEX relay_events_delivery_queue ON events(status, next_attempt_at, created_at);
CREATE INDEX relay_events_destination_created ON events(destination_id, created_at DESC);
CREATE INDEX relay_events_terminal_retention ON events(status, updated_at)
  WHERE status IN ('DELIVERED', 'EXPIRED');

CREATE TABLE delivery_attempts (
  id uuid PRIMARY KEY,
  event_id uuid NOT NULL REFERENCES events(id) ON DELETE CASCADE,
  status_code integer,
  response_body text,
  duration_ms bigint NOT NULL,
  error text,
  attempted_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX delivery_attempts_event ON delivery_attempts(event_id, attempted_at);

CREATE TABLE api_key_destinations (
  api_key_id uuid NOT NULL REFERENCES api_keys(id) ON DELETE CASCADE,
  destination_id uuid NOT NULL REFERENCES destinations(id) ON DELETE CASCADE,
  PRIMARY KEY (api_key_id, destination_id)
);

CREATE TABLE login_attempts (
  username_hash text PRIMARY KEY,
  failed_count integer NOT NULL DEFAULT 0,
  window_started_at timestamptz NOT NULL DEFAULT now(),
  blocked_until timestamptz
);
