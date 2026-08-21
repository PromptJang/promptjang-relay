use anyhow::{Context, Result, bail};
use sqlx::PgPool;
use uuid::Uuid;

use crate::config::Config;
use crate::domain::secrets::{hash_password, hash_secret};
use crate::domain::validation::bearer_token;
use axum::http::HeaderMap;

pub async fn bootstrap_owner(pool: &PgPool, config: &Config) -> Result<()> {
    let owner_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM owners)")
        .fetch_one(pool)
        .await?;
    if owner_exists {
        return Ok(());
    }
    let email = config
        .admin_email
        .as_deref()
        .context("PJ_ADMIN_EMAIL is required on first startup")?;
    let password = config
        .admin_password
        .as_deref()
        .context("PJ_ADMIN_PASSWORD is required on first startup")?;
    if password.len() < 12 {
        bail!("PJ_ADMIN_PASSWORD must contain at least 12 characters");
    }
    let password_hash = hash_password(password).map_err(|error| anyhow::anyhow!(error.to_string()))?;
    sqlx::query("INSERT INTO owners (id, email, password_hash) VALUES ($1, $2, $3)")
        .bind(Uuid::new_v4())
        .bind(email.to_lowercase())
        .bind(password_hash)
        .execute(pool)
        .await?;
    tracing::info!(email, "created the PromptJang Webhooks OSS owner");
    Ok(())
}

pub async fn require_session(headers: &HeaderMap, pool: &PgPool) -> Result<Uuid> {
    let raw = bearer_token(headers).context("session token required")?;
    if !raw.starts_with("pj_session_") {
        bail!("session token required");
    }
    let owner_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT owner_id FROM sessions WHERE token_hash = $1 AND expires_at > now()",
    )
    .bind(hash_secret(raw))
    .fetch_optional(pool)
    .await?
    .context("invalid or expired session")?;
    Ok(owner_id)
}

pub async fn require_api_key(headers: &HeaderMap, pool: &PgPool) -> Result<Uuid> {
    let raw = bearer_token(headers).context("API key required")?;
    if !raw.starts_with("pj_oss_") {
        bail!("API key required");
    }
    let id = sqlx::query_scalar::<_, Uuid>("SELECT id FROM api_keys WHERE secret_hash = $1")
        .bind(hash_secret(raw))
        .fetch_optional(pool)
        .await?
        .context("invalid API key")?;
    sqlx::query("UPDATE api_keys SET last_used_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(id)
}
