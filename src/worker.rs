use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::Client;
use serde_json::Value;
use sha2::Sha256;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const RETRY_DELAYS: [i64; 5] = [60, 120, 240, 480, 960];
const MAX_RESPONSE_BYTES: usize = 10_240;

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

pub fn retry_delay(retry_count: i32) -> Option<i64> {
    RETRY_DELAYS.get(retry_count.max(0) as usize).copied()
}

pub fn signature(secret: &str, timestamp: i64, payload: &[u8]) -> String {
    let mut signed = timestamp.to_string().into_bytes();
    signed.push(b'.');
    signed.extend_from_slice(payload);
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret.as_bytes())
        .expect("HMAC accepts arbitrary key lengths");
    mac.update(&signed);
    hex::encode(mac.finalize().into_bytes())
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
    let mut request = client
        .post(&destination.url)
        .header("Content-Type", "application/json")
        .header(
            "X-PromptJang-Signature",
            signature(&destination.signing_secret, timestamp, &payload),
        )
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
            let body = response.text().await.unwrap_or_default();
            let body = truncate(body);
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
        let delay = retry_delay(job.retry_count).unwrap_or(960);
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
        .execute(pool).await?;
    sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(pool)
        .await?;
    Ok(())
}

fn truncate(body: String) -> String {
    if body.len() <= MAX_RESPONSE_BYTES {
        return body;
    }
    let mut boundary = MAX_RESPONSE_BYTES;
    while !body.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}[truncated]", &body[..boundary])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_retry_schedule_matches_cloud() {
        assert_eq!(
            (0..5).filter_map(retry_delay).collect::<Vec<_>>(),
            vec![60, 120, 240, 480, 960]
        );
        assert_eq!(retry_delay(5), None);
    }

    #[test]
    fn signing_fixture_is_stable() {
        assert_eq!(
            signature("whsec_fixture", 1700000000, br#"{"ok":true}"#),
            "31a99e5c88be4311395a895ea0d686baf164714d49a52bae17fad334b78db984"
        );
    }
}
