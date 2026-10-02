# Agent Message Envelope v1

Relay and Relay One share an optional JSON Schema for task input and result output.
Opt in with `"schema": "promptjang.agent-message.v1"`. Plain text, arbitrary JSON,
and old unversioned envelopes remain accepted.

Task example:

```json
{"schema":"promptjang.agent-message.v1","kind":"task","correlation_id":"review-001","task":"Review the branch","reply_to":"results"}
```

Result example:

```json
{"schema":"promptjang.agent-message.v1","kind":"result","correlation_id":"review-001","in_reply_to":"550e8400-e29b-41d4-a716-446655440000","status":"succeeded","summary":"No blocking findings"}
```

Failed results require a sanitized `error`. Copy the correlation ID and use the
source message UUID for `in_reply_to`. Push results before ack with a stable
`result:SOURCE_MESSAGE_ID` idempotency key.

Pass the envelope as `mail_push.payload`, not a JSON string. Claim returns it in
`payload_json`. All five MCP tools advertise structured output schemas and
return structured content plus compatible text. The MCP wrapper is not the
task/result envelope. HTTP ingestion uses the same opt-in validation.

Invalid versioned envelopes are rejected before acceptance. Messages remain
untrusted data; no agent wake-up, loop, or execution authority is implied.

The canonical JSON Schema is in
`skills/promptjang/references/agent-message-v1.schema.json`.
