use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::domain::delivery::{fallback_delay, retry_delay, signature, truncate_body};

#[derive(FromRow)]
struct DeliveryJob {
    id: Uuid,
    endpoint_id: Uuid,
    payload: Value,
    event_type: Option<String>,
    correlation_id: Option<String>,
    retry_count: i32,
    max_retries: i32,
}

#[derive(FromRow)]
struct Destination {
    url: String,
    signing_secret: String,
    enabled: bool,
}

pub async fn run(pool: PgPool, client: Client) {
    loop {
        if let Err(error) = recover_stuck(&pool).await {
            tracing::error!(%error, "stuck delivery recovery failed");
        }
        match process_one(&pool, &client).await {
            Ok(true) => continue,
            Ok(false) => tokio::time::sleep(Duration::from_millis(500)).await,
            Err(error) => {
                tracing::error!(%error, "delivery worker iteration failed");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

async fn claim(pool: &PgPool) -> Result<Option<DeliveryJob>> {
    let mut tx = pool.begin().await?;
    let job = sqlx::query_as::<_, DeliveryJob>(
        "SELECT id, endpoint_id, payload, event_type, correlation_id, retry_count, max_retries
         FROM events
         WHERE status IN ('QUEUED','RETRYING') AND next_attempt_at <= now()
         ORDER BY next_attempt_at, created_at
         FOR UPDATE SKIP LOCKED LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(ref job) = job {
        sqlx::query("UPDATE events SET status='PROCESSING', updated_at=now() WHERE id=$1")
            .bind(job.id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(job)
}

async fn process_one(pool: &PgPool, client: &Client) -> Result<bool> {
    let Some(job) = claim(pool).await? else {
        return Ok(false);
    };
    let destination = sqlx::query_as::<_, Destination>(
        "SELECT url, signing_secret, enabled FROM endpoints WHERE id=$1",
    )
    .bind(job.endpoint_id)
    .fetch_one(pool)
    .await?;
    if !destination.enabled {
        return fail(pool, &job, None, "endpoint is disabled", 0, None)
            .await
            .map(|_| true);
    }

    let payload = serde_json::to_vec(&job.payload)?;
    let timestamp = Utc::now().timestamp();
    let started = Instant::now();
    let signed = signature(&destination.signing_secret, timestamp, &payload)
        .context("sign payload for delivery")?;
    let mut request = client
        .post(&destination.url)
        .header("Content-Type", "application/json")
        .header("X-PromptJang-Signature", signed)
        .header("X-PromptJang-Timestamp", timestamp)
        .header("X-PromptJang-Event-ID", job.id.to_string())
        .body(payload);
    if let Some(value) = &job.event_type {
        request = request.header("X-PromptJang-Event-Type", value);
    }
    if let Some(value) = &job.correlation_id {
        request = request.header("X-Correlation-ID", value);
    }

    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let body = truncate_body(response.text().await.unwrap_or_default());
            let duration = started.elapsed().as_millis() as i64;
            if status.is_success() {
                sqlx::query("INSERT INTO delivery_attempts (id,event_id,status_code,response_body,duration_ms) VALUES ($1,$2,$3,$4,$5)")
                    .bind(Uuid::new_v4()).bind(job.id).bind(i32::from(status.as_u16())).bind(body).bind(duration).execute(pool).await?;
                sqlx::query("UPDATE events SET status='DELIVERED', last_error=NULL, updated_at=now() WHERE id=$1")
                    .bind(job.id).execute(pool).await?;
            } else {
                fail(
                    pool,
                    &job,
                    Some(i32::from(status.as_u16())),
                    &format!("HTTP {status}"),
                    duration,
                    Some(body),
                )
                .await?;
            }
        }
        Err(error) => {
            fail(
                pool,
                &job,
                None,
                &error.to_string(),
                started.elapsed().as_millis() as i64,
                None,
            )
            .await?;
        }
    }
    Ok(true)
}

async fn fail(
    pool: &PgPool,
    job: &DeliveryJob,
    status: Option<i32>,
    error: &str,
    duration: i64,
    body: Option<String>,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO delivery_attempts (id,event_id,status_code,response_body,duration_ms,error) VALUES ($1,$2,$3,$4,$5,$6)")
        .bind(Uuid::new_v4()).bind(job.id).bind(status).bind(body).bind(duration).bind(error).execute(&mut *tx).await?;
    if job.retry_count < job.max_retries {
        let delay = retry_delay(job.retry_count).unwrap_or_else(fallback_delay);
        sqlx::query("UPDATE events SET status='RETRYING', retry_count=retry_count+1, next_attempt_at=now()+make_interval(secs => $2), last_error=$3, updated_at=now() WHERE id=$1")
            .bind(job.id).bind(delay as f64).bind(error).execute(&mut *tx).await?;
    } else {
        sqlx::query(
            "UPDATE events SET status='EXPIRED', last_error=$2, updated_at=now() WHERE id=$1",
        )
        .bind(job.id)
        .bind(error)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn recover_stuck(pool: &PgPool) -> Result<()> {
    sqlx::query("UPDATE events SET status='RETRYING', next_attempt_at=now(), last_error='recovered after interrupted delivery', updated_at=now() WHERE status='PROCESSING' AND updated_at < now() - interval '5 minutes'")
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(pool)
        .await?;
    Ok(())
}
