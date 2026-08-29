CREATE TABLE mailboxes (
  id uuid PRIMARY KEY,
  name text NOT NULL UNIQUE,
  created_at timestamptz NOT NULL DEFAULT now()
);

CREATE TABLE mailbox_messages (
  id uuid PRIMARY KEY,
  mailbox_id uuid NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
  status text NOT NULL CHECK (status IN ('UNREAD','CLAIMED','ACKNOWLEDGED')),
  payload_raw bytea NOT NULL,
  content_type text NOT NULL DEFAULT 'application/json',
  payload jsonb,
  payload_sha256 text NOT NULL,
  idempotency_key_hash text,
  traceparent text,
  tracestate text,
  claim_token text,
  claimed_until timestamptz,
  claim_count integer NOT NULL DEFAULT 0,
  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX relay_mail_idempotency_unique ON mailbox_messages(mailbox_id, idempotency_key_hash)
  WHERE idempotency_key_hash IS NOT NULL;
CREATE INDEX relay_mail_claim_queue ON mailbox_messages(mailbox_id, created_at)
  WHERE status = 'UNREAD';
CREATE INDEX relay_mail_lease_recovery ON mailbox_messages(mailbox_id, claimed_until)
  WHERE status = 'CLAIMED';
