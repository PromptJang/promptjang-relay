use uuid::Uuid;

use crate::domain::DomainError;
use crate::domain::models::EndpointView;
use crate::domain::validation::MAX_ENDPOINTS;
use sqlx::PgPool;

pub async fn list(pool: &PgPool) -> Result<Vec<EndpointView>, DomainError> {
    sqlx::query_as::<_, EndpointView>(
        "SELECT id,name,url,enabled,created_at,updated_at FROM endpoints ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(DomainError::from)
}

pub async fn create(
    pool: &PgPool,
    name: String,
    url: String,
    signing_secret: String,
    enabled: bool,
) -> Result<(Uuid, String), DomainError> {
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM endpoints")
        .fetch_one(pool)
        .await
        .map_err(DomainError::from)?;
    if count >= MAX_ENDPOINTS {
        return Err(DomainError::conflict("endpoint limit reached (10)"));
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO endpoints (id,name,url,signing_secret,enabled) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(id)
    .bind(name)
    .bind(url)
    .bind(&signing_secret)
    .bind(enabled)
    .execute(pool)
    .await
    .map_err(DomainError::from)?;
    Ok((id, signing_secret))
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    name: String,
    url: String,
    enabled: Option<bool>,
) -> Result<(), DomainError> {
    let changed =
        sqlx::query("UPDATE endpoints SET name=$2,url=$3,enabled=COALESCE($4,enabled),updated_at=now() WHERE id=$1")
            .bind(id)
            .bind(name)
            .bind(url)
            .bind(enabled)
            .execute(pool)
            .await
            .map_err(DomainError::from)?
            .rows_affected();
    if changed == 0 {
        return Err(DomainError::not_found("endpoint not found"));
    }
    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), DomainError> {
    let changed = sqlx::query("DELETE FROM endpoints WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(DomainError::from)?
        .rows_affected();
    if changed == 0 {
        return Err(DomainError::not_found("endpoint not found"));
    }
    Ok(())
}
