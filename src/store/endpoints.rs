use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::DomainError;
use crate::domain::models::DestinationView;
use crate::domain::secrets;

pub async fn migrate_plaintext_secrets(pool: &PgPool, key: &[u8; 32]) -> Result<u64, DomainError> {
    let rows = sqlx::query_as::<_, (Uuid, String)>(
        "SELECT id,signing_secret FROM destinations WHERE signing_secret_ciphertext IS NULL",
    )
    .fetch_all(pool)
    .await?;
    let mut migrated = 0;
    for (id, plaintext) in rows {
        let ciphertext = secrets::encrypt_secret(key, &plaintext)?;
        migrated += sqlx::query("UPDATE destinations SET signing_secret_ciphertext=$2, signing_secret='' WHERE id=$1 AND signing_secret_ciphertext IS NULL")
            .bind(id).bind(ciphertext).execute(pool).await?.rows_affected();
    }
    Ok(migrated)
}

pub async fn list(pool: &PgPool) -> Result<Vec<DestinationView>, DomainError> {
    sqlx::query_as::<_, DestinationView>("SELECT id,name,url,enabled,created_at,updated_at,deleted_at,(previous_signing_secret_ciphertext IS NOT NULL) AS has_previous_secret FROM destinations WHERE deleted_at IS NULL ORDER BY created_at DESC")
        .fetch_all(pool).await.map_err(DomainError::from)
}

pub async fn create(
    pool: &PgPool,
    key: &[u8; 32],
    name: String,
    url: String,
    signing_secret: String,
    enabled: bool,
) -> Result<(Uuid, String), DomainError> {
    let id = Uuid::new_v4();
    let ciphertext = secrets::encrypt_secret(key, &signing_secret)?;
    sqlx::query("INSERT INTO destinations (id,name,url,signing_secret,signing_secret_ciphertext,enabled) VALUES ($1,$2,$3,'',$4,$5)")
        .bind(id).bind(name).bind(url).bind(ciphertext).bind(enabled).execute(pool).await?;
    Ok((id, signing_secret))
}

pub async fn update(
    pool: &PgPool,
    id: Uuid,
    name: String,
    url: String,
    enabled: Option<bool>,
) -> Result<(), DomainError> {
    let changed = sqlx::query("UPDATE destinations SET name=$2,url=$3,enabled=COALESCE($4,enabled),updated_at=now() WHERE id=$1 AND deleted_at IS NULL")
        .bind(id).bind(name).bind(url).bind(enabled).execute(pool).await?.rows_affected();
    if changed == 0 {
        return Err(DomainError::not_found("destination not found"));
    }
    Ok(())
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), DomainError> {
    let changed = sqlx::query("UPDATE destinations SET deleted_at=now(),enabled=false,updated_at=now() WHERE id=$1 AND deleted_at IS NULL")
        .bind(id).execute(pool).await?.rows_affected();
    if changed == 0 {
        return Err(DomainError::not_found("destination not found"));
    }
    Ok(())
}

pub async fn rotate_secret(pool: &PgPool, key: &[u8; 32], id: Uuid) -> Result<String, DomainError> {
    let secret = secrets::new_secret("whsec_");
    let ciphertext = secrets::encrypt_secret(key, &secret)?;
    let changed = sqlx::query("UPDATE destinations SET previous_signing_secret_ciphertext=signing_secret_ciphertext, signing_secret_ciphertext=$2, updated_at=now() WHERE id=$1 AND deleted_at IS NULL")
        .bind(id).bind(ciphertext).execute(pool).await?.rows_affected();
    if changed == 0 {
        return Err(DomainError::not_found("destination not found"));
    }
    Ok(secret)
}

pub async fn finish_rotation(pool: &PgPool, id: Uuid) -> Result<(), DomainError> {
    let changed = sqlx::query("UPDATE destinations SET previous_signing_secret_ciphertext=NULL,updated_at=now() WHERE id=$1 AND deleted_at IS NULL")
        .bind(id).execute(pool).await?.rows_affected();
    if changed == 0 {
        return Err(DomainError::not_found("destination not found"));
    }
    Ok(())
}
