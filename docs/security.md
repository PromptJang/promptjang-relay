# Relay security

Relay is designed for one trusted team. Put the UI and administration API behind TLS and network access controls. A reverse proxy may provide SSO, but Relay v0.2 still uses its local owner session.

- Signing secrets are AES-GCM encrypted with `PJ_ENCRYPTION_KEY`.
- API keys and sessions are stored only as SHA-256 hashes; owner passwords use Argon2id.
- Store the encryption key, bootstrap password, database password, and OTLP headers in a secret manager.
- Back up the encryption key separately. Losing it makes destination secrets unrecoverable.
- Public HTTPS destinations work by default. Private addresses require explicit CIDRs and are revalidated before delivery.
- HTTP is limited to allowlisted private destinations and requires `PJ_ALLOW_INSECURE_HTTP=true`.
- Redirects are disabled. Embedded URL credentials are rejected.
- Relay never records or exports payloads, authorization headers, cookies, secrets, encryption keys, database credentials, or OTLP authorization headers as telemetry.
- Receivers must verify signatures and tolerate duplicate delivery.

