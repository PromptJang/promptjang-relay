# Upgrade from v0.1 to v0.2

v0.2 automatically renames endpoints to destinations, preserves events and attempts, backfills stored payload bytes, encrypts existing signing secrets, and adds API-key scoping. Existing `pj_oss_` keys and v0.1 routes continue working.

Before starting v0.2:

1. Back up PostgreSQL.
2. Create and securely store a base64-encoded 32-byte `PJ_ENCRYPTION_KEY`.
3. Update the image and environment configuration.
4. Start one instance and allow automatic migrations to finish.
5. Verify destinations and event history before scaling workers.

Historical v0.1 payloads are reconstructed from stored JSON because v0.1 did not retain original request bytes. Events accepted after v0.2 preserve exact bytes.
