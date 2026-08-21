use uuid::Uuid;

use crate::domain::DomainError;
use crate::domain::models::ApiKeyView;
use crate::domain::validation::MAX_KEYS;
use sqlx::PgPool;

pub async fn list(pool: &PgPool) -> Result<Vec<ApiKeyView>, DomainError> {
    sqlx::query_as::<_, ApiKeyView>(
        "SELECT id,name,prefix,last_used_at,created_at FROM api_keys ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(DomainError::from)
}

pub async fn create(pool: &PgPool, name: String) -> Result<(Uuid, String), DomainError> {
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM api_keys")
        .fetch_one(pool)
        .await
        .map_err(DomainError::from)?;
    if count >= MAX_KEYS {
        return Err(DomainError::conflict("API key limit reached (5)"));
    }
    let id = Uuid::new_v4();
    let secret = crate::domain::secrets::new_secret("pj_oss_");
    let prefix = secret.chars().take(15).collect::<String>();
    sqlx::query("INSERT INTO api_keys (id,name,prefix,secret_hash) VALUES ($1,$2,$3,$4)")
        .bind(id)
        .bind(name)
        .bind(prefix)
        .bind(crate::domain::secrets::hash_secret(&secret))
        .execute(pool)
        .await
        .map_err(DomainError::from)?;
    Ok((id, secret))
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), DomainError> {
    sqlx::query("DELETE FROM api_keys WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(DomainError::from)?;
    Ok(())
}
