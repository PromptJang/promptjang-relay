CREATE TABLE mcp_sessions (
  session_id text PRIMARY KEY,
  initialize_state jsonb NOT NULL,
  created_at timestamptz NOT NULL DEFAULT now(),
  expires_at timestamptz NOT NULL
);

CREATE INDEX relay_mcp_sessions_expiry ON mcp_sessions(expires_at);
