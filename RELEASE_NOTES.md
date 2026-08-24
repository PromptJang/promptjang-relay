# PromptJang Relay v0.3.0

Relay v0.3 adopts Standard Webhooks v1 for interoperable outbound signing. Relay remains a focused PostgreSQL-native webhook delivery product.

## Highlights

- Standard `webhook-id`, `webhook-timestamp`, and `webhook-signature` delivery headers.
- Stable signed event IDs across retries and new identities for replay.
- Current and previous rotation signatures in one Standard Webhooks header.
- Base64-encoded 256-bit signing secrets for new destinations and rotations.
- Cross-language compatibility verification against the official JavaScript implementation.
- Existing delivery, PostgreSQL, security, UI, and optional OpenTelemetry behavior preserved.

This release changes the receiver signing contract. Review [the v0.2 to v0.3 migration guide](docs/migration-v02-v03.md) before upgrading.
