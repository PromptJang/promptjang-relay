# External PostgreSQL deployment

Relay depends only on `DATABASE_URL`; the database in `compose.yaml` is an example. RDS, Cloud SQL, Neon, Supabase, and other compatible PostgreSQL services can replace it.

Run the application container independently with a TLS-enabled PostgreSQL URL, owner bootstrap values for the first start, and `PJ_ENCRYPTION_KEY`. Restrict database access to Relay, use a dedicated database role, cap `PJ_DB_MAX_CONNECTIONS` below the provider limit, and terminate public HTTP at a reverse proxy with TLS.

Relay applies SQLx migrations at startup. During upgrades, start one new instance first, wait for `/ready`, verify a test delivery, and then replace remaining instances. PostgreSQL upgrades should be rehearsed against a restored backup with the same Relay image and encryption key.

Back up database data and `PJ_ENCRYPTION_KEY` through separate systems. A database restore without the matching key preserves history but cannot decrypt signing secrets.
