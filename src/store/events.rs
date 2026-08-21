use serde_json::Value;
use uuid::Uuid;

use crate::domain::DomainError;
use crate::domain::models::{AttemptView, EventView};
use crate::domain::validation::PER_MINUTE_EVENTS;
use sqlx::PgPool;

pub enum IngestOutcome {
    Created { id: Uuid },
    IdempotentReplay { id: Uuid, status: String },
}

pub async fn list(pool: &PgPool, limit: i64) -> Result<Vec<EventView>, DomainError> {
    sqlx::query_as::<_, EventView>(
        "SELECT id,endpoint_id,status,event_type,correlation_id,payload,retry_count,max_retries,is_replay,source_event_id,next_attempt_at,last_error,created_at,updated_at FROM events ORDER BY created_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(DomainError::from)
}

pub async fn get(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<(EventView, Vec<AttemptView>)>, DomainError> {
    let event = sqlx::query_as::<_, EventView>(
        "SELECT id,endpoint_id,status,event_type,correlation_id,payload,retry_count,max_retries,is_replay,source_event_id,next_attempt_at,last_error,created_at,updated_at FROM events WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(DomainError::from)?;
    let Some(event) = event else {
        return Ok(None);
    };
    let attempts = sqlx::query_as::<_, AttemptView>(
        "SELECT id,event_id,status_code,response_body,duration_ms,error,attempted_at FROM delivery_attempts WHERE event_id=$1 ORDER BY attempted_at",
    )
    .bind(id)
    .fetch_all(pool)
    .await
    .map_err(DomainError::from)?;
    Ok(Some((event, attempts)))
}

pub async fn replay(pool: &PgPool, source_id: Uuid) -> Result<Uuid, DomainError> {
    let source = sqlx::query_as::<_, (Uuid, Value, String, Option<String>, Option<String>)>(
        "SELECT endpoint_id,payload,payload_sha256,event_type,correlation_id FROM events WHERE id=$1",
    )
    .bind(source_id)
    .fetch_optional(pool)
    .await
    .map_err(DomainError::from)?;
    let Some(source) = source else {
        return Err(DomainError::not_found("event not found"));
    };
    let replay_id = Uuid::new_v4();
    sqlx::query("INSERT INTO events (id,endpoint_id,status,payload,payload_sha256,event_type,correlation_id,is_replay,source_event_id) VALUES ($1,$2,'QUEUED',$3,$4,$5,$6,true,$7)")
        .bind(replay_id)
        .bind(source.0)
        .bind(source.1)
        .bind(source.2)
        .bind(source.3)
        .bind(source.4)
        .bind(source_id)
        .execute(pool)
        .await
        .map_err(DomainError::from)?;
    Ok(replay_id)
}

#[allow(clippy::too_many_arguments)]
pub async fn ingest(
    pool: &PgPool,
    endpoint_id: Uuid,
    payload: Value,
    payload_hash: String,
    key_hash: Option<String>,
    event_type: Option<String>,
    correlation_id: Option<String>,
) -> Result<IngestOutcome, DomainError> {
    let enabled = sqlx::query_scalar::<_, bool>("SELECT enabled FROM endpoints WHERE id=$1")
        .bind(endpoint_id)
        .fetch_optional(pool)
        .await
        .map_err(DomainError::from)?
        .ok_or_else(|| DomainError::not_found("endpoint not found"))?;
    if !enabled {
        return Err(DomainError::conflict("endpoint is disabled"));
    }
    let minute_count =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM events WHERE endpoint_id=$1 AND is_replay=false AND created_at>=date_trunc('minute',now())")
            .bind(endpoint_id)
            .fetch_one(pool)
            .await
            .map_err(DomainError::from)?;
    if minute_count >= PER_MINUTE_EVENTS {
        return Err(DomainError::too_many_requests(
            "accepted-event limit reached",
        ));
    }

    let mut tx = pool.begin().await.map_err(DomainError::from)?;
    if let Some(ref hash) = key_hash {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("{endpoint_id}:{hash}"))
            .execute(&mut *tx)
            .await
            .map_err(DomainError::from)?;
        let existing = sqlx::query_as::<_, (Uuid, String, String)>(
            "SELECT id,payload_sha256,status FROM events WHERE endpoint_id=$1 AND idempotency_key_hash=$2 AND is_replay=false",
        )
        .bind(endpoint_id)
        .bind(hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(DomainError::from)?;
        if let Some((existing_id, existing_hash, status)) = existing {
            if existing_hash != payload_hash {
                return Err(DomainError::conflict(
                    "Idempotency-Key was already used with a different payload",
                ));
            }
            tx.commit().await.map_err(DomainError::from)?;
            return Ok(IngestOutcome::IdempotentReplay {
                id: existing_id,
                status,
            });
        }
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO events (id,endpoint_id,status,event_type,correlation_id,payload,payload_sha256,idempotency_key_hash) VALUES ($1,$2,'QUEUED',$3,$4,$5,$6,$7)")
        .bind(id)
        .bind(endpoint_id)
        .bind(event_type)
        .bind(correlation_id)
        .bind(payload)
        .bind(payload_hash)
        .bind(key_hash)
        .execute(&mut *tx)
        .await
        .map_err(DomainError::from)?;
    tx.commit().await.map_err(DomainError::from)?;
    Ok(IngestOutcome::Created { id })
}
