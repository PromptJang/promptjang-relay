CREATE TABLE owners (
  id uuid PRIMARY KEY,
  email text NOT NULL UNIQUE,
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
  last_used_at timestamptz,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE endpoints (
  id uuid PRIMARY KEY,
  name text NOT NULL,
  url text NOT NULL,
  signing_secret text NOT NULL,
  enabled boolean NOT NULL DEFAULT true,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE events (
  id uuid PRIMARY KEY,
  endpoint_id uuid NOT NULL REFERENCES endpoints(id) ON DELETE CASCADE,
  status text NOT NULL CHECK (status IN ('QUEUED','PROCESSING','DELIVERED','RETRYING','EXPIRED')),
  event_type text,
  correlation_id text,
  payload jsonb NOT NULL,
  payload_sha256 text NOT NULL,
  idempotency_key_hash text,
  retry_count integer NOT NULL DEFAULT 0,
  max_retries integer NOT NULL DEFAULT 5,
  is_replay boolean NOT NULL DEFAULT false,
  source_event_id uuid REFERENCES events(id) ON DELETE SET NULL,
  next_attempt_at timestamptz NOT NULL DEFAULT now(),
  last_error text,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX events_idempotency_unique ON events(endpoint_id, idempotency_key_hash)
  WHERE idempotency_key_hash IS NOT NULL AND is_replay = false;
CREATE INDEX events_delivery_queue ON events(status, next_attempt_at, created_at);

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
