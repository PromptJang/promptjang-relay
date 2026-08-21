use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::DomainError;
use crate::domain::models::ApiKeyView;

pub async fn list(pool: &PgPool) -> Result<Vec<ApiKeyView>, DomainError> {
    let rows = sqlx::query_as::<_, (Uuid, String, String, Option<chrono::DateTime<chrono::Utc>>, chrono::DateTime<chrono::Utc>, bool)>(
        "SELECT id,name,prefix,last_used_at,created_at,unrestricted FROM api_keys ORDER BY created_at DESC",
    ).fetch_all(pool).await?;
    let mut result = Vec::with_capacity(rows.len());
    for (id, name, prefix, last_used_at, created_at, unrestricted) in rows {
        let destination_ids = sqlx::query_scalar::<_, Uuid>(
            "SELECT destination_id FROM api_key_destinations WHERE api_key_id=$1 ORDER BY destination_id",
        ).bind(id).fetch_all(pool).await?;
        result.push(ApiKeyView {
            id,
            name,
            prefix,
            last_used_at,
            created_at,
            unrestricted,
            destination_ids,
        });
    }
    Ok(result)
}

pub async fn create(
    pool: &PgPool,
    name: String,
    destination_ids: Vec<Uuid>,
) -> Result<(Uuid, String), DomainError> {
    let id = Uuid::new_v4();
    let secret = crate::domain::secrets::new_secret("pj_relay_");
    let prefix = secret.chars().take(18).collect::<String>();
    let unrestricted = destination_ids.is_empty();
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO api_keys (id,name,prefix,secret_hash,unrestricted) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(id)
    .bind(name)
    .bind(prefix)
    .bind(crate::domain::secrets::hash_secret(&secret))
    .bind(unrestricted)
    .execute(&mut *tx)
    .await?;
    for destination_id in destination_ids {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM destinations WHERE id=$1 AND deleted_at IS NULL)",
        )
        .bind(destination_id)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(DomainError::not_found("destination scope not found"));
        }
        sqlx::query("INSERT INTO api_key_destinations (api_key_id,destination_id) VALUES ($1,$2)")
            .bind(id)
            .bind(destination_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok((id, secret))
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), DomainError> {
    let changed = sqlx::query("DELETE FROM api_keys WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(DomainError::not_found("API key not found"));
    }
    Ok(())
}
