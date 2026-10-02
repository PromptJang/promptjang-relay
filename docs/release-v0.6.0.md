# Relay v0.6.0

- Optional Agent Message Envelope v1 for structured task/result payloads.
- MCP tools expose input and output schemas for compatible clients.
- Plain text, arbitrary JSON, and unversioned messages remain supported.
- Explicit versioned envelopes are validated before durable acceptance.
- No database migration or MCP connection configuration change is required.

Existing mailboxes, API keys, webhook signing, retries, and telemetry are unchanged.
