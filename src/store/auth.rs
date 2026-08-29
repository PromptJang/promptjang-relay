use anyhow::{Context, Result, bail};
use axum::http::HeaderMap;
use sqlx::PgPool;
use std::sync::OnceLock;
use uuid::Uuid;

use crate::config::Config;
use crate::domain::secrets::{hash_password, hash_secret};
use crate::domain::validation::bearer_token;

static DUMMY_PASSWORD_HASH: OnceLock<String> = OnceLock::new();

fn dummy_password_hash() -> Result<&'static str> {
    if let Some(value) = DUMMY_PASSWORD_HASH.get() {
        return Ok(value);
    }
    let value = hash_password("relay-constant-cost-dummy-password")
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let _ = DUMMY_PASSWORD_HASH.set(value);
    Ok(DUMMY_PASSWORD_HASH
        .get()
        .expect("dummy password hash was initialized"))
}

pub async fn bootstrap_owner(pool: &PgPool, config: &Config) -> Result<()> {
    let username = config
        .admin_username
        .as_deref()
        .context("PJ_ADMIN_USERNAME is required on first startup")?
        .to_lowercase();
    let password = config
        .admin_password
        .as_deref()
        .context("PJ_ADMIN_PASSWORD is required on first startup")?;
    let min_len = if config.allow_weak_password { 1 } else { 12 };
    if password.len() < min_len {
        bail!("PJ_ADMIN_PASSWORD must contain at least {min_len} characters");
    }
    let password_hash =
        hash_password(password).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    let changed = sqlx::query(
        "INSERT INTO owners (id,username,password_hash) VALUES ($1,$2,$3)
         ON CONFLICT (username) DO UPDATE SET password_hash=$3
         WHERE owners.password_hash IS DISTINCT FROM $3
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(&username)
    .bind(&password_hash)
    .fetch_optional(pool)
    .await?;
    if changed.is_some() {
        sqlx::query(
            "DELETE FROM sessions WHERE owner_id=(SELECT id FROM owners WHERE username=$1)",
        )
        .bind(&username)
        .execute(pool)
        .await?;
        tracing::info!(username, "set the PromptJang Relay owner credentials");
    }
    Ok(())
}

pub async fn verify_login(pool: &PgPool, username: &str, password: &str) -> Result<Option<Uuid>> {
    let username_hash = hash_secret(&username.to_lowercase());
    let blocked = sqlx::query_scalar::<_, bool>(
        "SELECT COALESCE(blocked_until > now(),false) FROM login_attempts WHERE username_hash=$1",
    )
    .bind(&username_hash)
    .fetch_optional(pool)
    .await?
    .unwrap_or(false);
    if blocked {
        return Ok(None);
    }
    let owner = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id,password_hash FROM owners WHERE username=$1",
    )
    .bind(username.to_lowercase())
    .fetch_optional(pool)
    .await?;
    let fallback = dummy_password_hash()?;
    let verified = owner.as_ref().map_or_else(
        || crate::domain::secrets::verify_password(password, fallback),
        |(_, encoded)| crate::domain::secrets::verify_password(password, encoded),
    );
    if verified {
        sqlx::query("DELETE FROM login_attempts WHERE username_hash=$1")
            .bind(username_hash)
            .execute(pool)
            .await?;
        return Ok(owner.map(|(id, _)| id));
    }
    sqlx::query("INSERT INTO login_attempts(username_hash,failed_count,window_started_at,blocked_until) VALUES($1,1,now(),NULL) ON CONFLICT(username_hash) DO UPDATE SET failed_count=CASE WHEN login_attempts.window_started_at < now()-interval '15 minutes' THEN 1 ELSE login_attempts.failed_count+1 END, window_started_at=CASE WHEN login_attempts.window_started_at < now()-interval '15 minutes' THEN now() ELSE login_attempts.window_started_at END, blocked_until=CASE WHEN login_attempts.failed_count+1>=5 THEN now()+interval '15 minutes' ELSE login_attempts.blocked_until END")
        .bind(username_hash).execute(pool).await?;
    Ok(None)
}

pub async fn issue_session(pool: &PgPool, owner_id: Uuid, ttl_seconds: i64) -> Result<String> {
    let token = crate::domain::secrets::new_secret("pj_session_");
    sqlx::query("INSERT INTO sessions(id,owner_id,token_hash,expires_at) VALUES($1,$2,$3,now()+make_interval(secs=>$4))")
        .bind(Uuid::new_v4()).bind(owner_id).bind(hash_secret(&token)).bind(ttl_seconds as f64).execute(pool).await?;
    Ok(token)
}

pub async fn revoke_session(headers: &HeaderMap, pool: &PgPool) -> Result<()> {
    let raw = bearer_token(headers).context("session token required")?;
    sqlx::query("DELETE FROM sessions WHERE token_hash=$1")
        .bind(hash_secret(raw))
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn require_session(headers: &HeaderMap, pool: &PgPool) -> Result<Uuid> {
    let raw = bearer_token(headers).context("session token required")?;
    if !raw.starts_with("pj_session_") {
        bail!("session token required");
    }
    sqlx::query_scalar::<_, Uuid>(
        "SELECT owner_id FROM sessions WHERE token_hash=$1 AND expires_at>now()",
    )
    .bind(hash_secret(raw))
    .fetch_optional(pool)
    .await?
    .context("invalid or expired session")
}

pub async fn require_api_key(
    headers: &HeaderMap,
    pool: &PgPool,
    destination_id: Uuid,
) -> Result<Uuid> {
    require_scoped_api_key(headers, pool, Some(destination_id)).await
}

pub async fn require_unscoped_api_key(headers: &HeaderMap, pool: &PgPool) -> Result<Uuid> {
    require_scoped_api_key(headers, pool, None).await
}

async fn require_scoped_api_key(
    headers: &HeaderMap,
    pool: &PgPool,
    destination_id: Option<Uuid>,
) -> Result<Uuid> {
    let raw = bearer_token(headers).context("API key required")?;
    if !(raw.starts_with("pj_relay_") || raw.starts_with("pj_oss_")) {
        bail!("API key required");
    }
    let row = sqlx::query_as::<_, (Uuid, bool)>(
        "SELECT id,unrestricted FROM api_keys WHERE secret_hash=$1",
    )
    .bind(hash_secret(raw))
    .fetch_optional(pool)
    .await?
    .context("invalid API key")?;
    if let Some(destination_id) = destination_id.filter(|_| !row.1) {
        let allowed = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM api_key_destinations WHERE api_key_id=$1 AND destination_id=$2)")
            .bind(row.0).bind(destination_id).fetch_one(pool).await?;
        if !allowed {
            bail!("API key is not authorized for this destination");
        }
    }
    sqlx::query("UPDATE api_keys SET last_used_at=now() WHERE id=$1")
        .bind(row.0)
        .execute(pool)
        .await?;
    Ok(row.0)
}
