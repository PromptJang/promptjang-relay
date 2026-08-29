use serde_json::Value;
use uuid::Uuid;

use crate::domain::DomainError;
use crate::domain::secrets;
use sqlx::PgPool;

pub const DEFAULT_LEASE_SECONDS: i64 = 300;
pub const MIN_LEASE_SECONDS: i64 = 30;
pub const MAX_LEASE_SECONDS: i64 = 3600;
pub const DEFAULT_CLAIM_LIMIT: i64 = 10;
pub const MAX_CLAIM_LIMIT: i64 = 100;
pub const MAX_MAILBOX_NAME_CHARS: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    pub seconds: i64,
}

pub fn normalize_lease(seconds: Option<i64>) -> Lease {
    Lease {
        seconds: seconds
            .unwrap_or(DEFAULT_LEASE_SECONDS)
            .clamp(MIN_LEASE_SECONDS, MAX_LEASE_SECONDS),
    }
}

pub fn normalize_claim_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(DEFAULT_CLAIM_LIMIT)
        .clamp(1, MAX_CLAIM_LIMIT)
}

pub fn validate_mailbox_name(name: &str) -> Result<(), DomainError> {
    let valid = !name.trim().is_empty()
        && name.len() <= MAX_MAILBOX_NAME_CHARS
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || character == '-'
                || character == '_'
                || character == '.'
        });
    if valid {
        Ok(())
    } else {
        Err(DomainError::bad_request(
            "mailbox name must contain 1 to 100 characters of a-z, A-Z, 0-9, '-', '_', or '.'",
        ))
    }
}

pub fn new_claim_token() -> String {
    secrets::new_secret("mlc_")
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct MailboxMessage {
    pub id: Uuid,
    pub status: String,
    pub content_type: String,
    pub payload: Value,
    pub payload_raw: Vec<u8>,
    pub payload_sha256: String,
    pub traceparent: Option<String>,
    pub claim_token: Option<String>,
    pub claim_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub enum MailPushOutcome {
    Created { id: Uuid },
    IdempotentReplay { id: Uuid },
}

pub struct IncomingMessage {
    pub payload_raw: Vec<u8>,
    pub payload: Option<Value>,
    pub content_type: String,
    pub payload_sha256: String,
    pub idempotency_key_hash: Option<String>,
    pub traceparent: Option<String>,
    pub tracestate: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct MailboxId {
    id: Uuid,
}

async fn mailbox_id(pool: &PgPool, name: &str) -> Result<MailboxId, DomainError> {
    sqlx::query_as::<_, MailboxId>("SELECT id FROM mailboxes WHERE name=$1")
        .bind(name)
        .fetch_optional(pool)
        .await
        .map_err(DomainError::from)?
        .ok_or_else(|| DomainError::not_found("mailbox not found"))
}

pub async fn push(
    pool: &PgPool,
    name: &str,
    message: IncomingMessage,
) -> Result<MailPushOutcome, DomainError> {
    let IncomingMessage {
        payload_raw,
        payload,
        content_type,
        payload_sha256,
        idempotency_key_hash: key_hash,
        traceparent,
        tracestate,
    } = message;
    let mut tx = pool.begin().await.map_err(DomainError::from)?;
    let mailbox = sqlx::query_as::<_, MailboxId>(
        "INSERT INTO mailboxes (id,name) VALUES ($1,$2)
         ON CONFLICT (name) DO UPDATE SET name=EXCLUDED.name
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(name)
    .fetch_one(&mut *tx)
    .await
    .map_err(DomainError::from)?;
    if let Some(ref hash) = key_hash {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("mail:{}:{hash}", mailbox.id))
            .execute(&mut *tx)
            .await
            .map_err(DomainError::from)?;
        let existing = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM mailbox_messages WHERE mailbox_id=$1 AND idempotency_key_hash=$2",
        )
        .bind(mailbox.id)
        .bind(hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(DomainError::from)?;
        if let Some(existing_id) = existing {
            tx.commit().await.map_err(DomainError::from)?;
            return Ok(MailPushOutcome::IdempotentReplay { id: existing_id });
        }
    }
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO mailbox_messages
         (id,mailbox_id,status,payload_raw,content_type,payload,payload_sha256,idempotency_key_hash,traceparent,tracestate)
         VALUES ($1,$2,'UNREAD',$3,$4,$5,$6,$7,$8,$9)",
    )
    .bind(id)
    .bind(mailbox.id)
    .bind(&payload_raw)
    .bind(&content_type)
    .bind(payload)
    .bind(&payload_sha256)
    .bind(key_hash)
    .bind(traceparent)
    .bind(tracestate)
    .execute(&mut *tx)
    .await
    .map_err(DomainError::from)?;
    tx.commit().await.map_err(DomainError::from)?;
    Ok(MailPushOutcome::Created { id })
}

pub async fn claim(
    pool: &PgPool,
    name: &str,
    limit: i64,
    lease_seconds: i64,
) -> Result<Vec<MailboxMessage>, DomainError> {
    let mailbox = mailbox_id(pool, name).await?;
    let mut tx = pool.begin().await.map_err(DomainError::from)?;
    let claimed = sqlx::query_as::<_, MailboxMessage>(
        "WITH next_messages AS (
           SELECT id FROM mailbox_messages
           WHERE mailbox_id=$1 AND (status='UNREAD' OR (status='CLAIMED' AND claimed_until < now()))
           ORDER BY created_at
           FOR UPDATE SKIP LOCKED LIMIT $2
         )
         UPDATE mailbox_messages m
         SET status='CLAIMED', claim_token=$3, claimed_until=now()+make_interval(secs=>$4),
             claim_count=m.claim_count+1, updated_at=now()
         FROM next_messages
         WHERE m.id=next_messages.id
         RETURNING m.id, m.status, m.content_type, m.payload, m.payload_raw, m.payload_sha256,
                   m.traceparent, m.claim_token, m.claim_count, m.created_at, m.updated_at",
    )
    .bind(mailbox.id)
    .bind(limit)
    .bind(new_claim_token())
    .bind(lease_seconds)
    .fetch_all(&mut *tx)
    .await
    .map_err(DomainError::from)?;
    tx.commit().await.map_err(DomainError::from)?;
    Ok(claimed)
}

pub async fn acknowledge(
    pool: &PgPool,
    name: &str,
    id: Uuid,
    claim_token: &str,
    acknowledge: bool,
) -> Result<bool, DomainError> {
    let mailbox = mailbox_id(pool, name).await?;
    let next_status = if acknowledge {
        "ACKNOWLEDGED"
    } else {
        "UNREAD"
    };
    let changed = sqlx::query(&format!(
        "UPDATE mailbox_messages SET status='{next_status}', claim_token=NULL, claimed_until=NULL, updated_at=now()
         WHERE id=$1 AND mailbox_id=$2 AND claim_token=$3 AND status='CLAIMED'"
    ))
    .bind(id)
    .bind(mailbox.id)
    .bind(claim_token)
    .execute(pool)
    .await
    .map_err(DomainError::from)?
    .rows_affected();
    Ok(changed > 0)
}

#[derive(Debug, sqlx::FromRow, serde::Serialize)]
pub struct MailboxSummary {
    pub name: String,
    pub unread: i64,
    pub claimed: i64,
    pub acknowledged: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_mailboxes(pool: &PgPool) -> Result<Vec<MailboxSummary>, DomainError> {
    sqlx::query_as::<_, MailboxSummary>(
        "SELECT m.name,
                COALESCE(count(*) FILTER (WHERE g.status='UNREAD'),0) AS unread,
                COALESCE(count(*) FILTER (WHERE g.status='CLAIMED'),0) AS claimed,
                COALESCE(count(*) FILTER (WHERE g.status='ACKNOWLEDGED'),0) AS acknowledged,
                m.created_at
         FROM mailboxes m
         LEFT JOIN mailbox_messages g ON g.mailbox_id=m.id
         GROUP BY m.id, m.name, m.created_at
         ORDER BY m.created_at",
    )
    .fetch_all(pool)
    .await
    .map_err(DomainError::from)
}

pub async fn list_messages(
    pool: &PgPool,
    name: &str,
    status: Option<&str>,
    limit: i64,
) -> Result<Vec<MailboxMessage>, DomainError> {
    let mailbox = mailbox_id(pool, name).await?;
    sqlx::query_as::<_, MailboxMessage>(
        "SELECT id,status,content_type,payload,payload_raw,payload_sha256,traceparent,claim_token,claim_count,created_at,updated_at
         FROM mailbox_messages
         WHERE mailbox_id=$1 AND ($2::text IS NULL OR status=$2::text)
         ORDER BY created_at DESC LIMIT $3",
    )
    .bind(mailbox.id)
    .bind(status)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(DomainError::from)
}

pub async fn delete_mailbox(pool: &PgPool, name: &str) -> Result<bool, DomainError> {
    let changed = sqlx::query("DELETE FROM mailboxes WHERE name=$1")
        .bind(name)
        .execute(pool)
        .await
        .map_err(DomainError::from)?
        .rows_affected();
    Ok(changed > 0)
}

pub async fn cleanup(pool: &PgPool, retention_days: i64) -> Result<u64, DomainError> {
    if retention_days <= 0 {
        return Ok(0);
    }
    Ok(sqlx::query(
        "DELETE FROM mailbox_messages WHERE status='ACKNOWLEDGED' AND updated_at < now()-make_interval(days=>$1)",
    )
    .bind(retention_days as i32)
    .execute(pool)
    .await
    .map_err(DomainError::from)?
    .rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_defaults_and_clamps_to_documented_bounds() {
        // Arrange
        let cases = [
            (None, DEFAULT_LEASE_SECONDS),
            (Some(10), MIN_LEASE_SECONDS),
            (Some(999_999), MAX_LEASE_SECONDS),
            (Some(600), 600),
        ];

        for (input, expected) in cases {
            // Act
            let lease = normalize_lease(input);

            // Assert
            assert_eq!(lease.seconds, expected);
        }
    }

    #[test]
    fn claim_limit_clamps_into_safe_range() {
        // Arrange
        let cases = [
            (None, DEFAULT_CLAIM_LIMIT),
            (Some(0), 1),
            (Some(5_000), MAX_CLAIM_LIMIT),
        ];

        for (input, expected) in cases {
            // Act
            let limit = normalize_claim_limit(input);

            // Assert
            assert_eq!(limit, expected);
        }
    }

    #[test]
    fn mailbox_names_accept_tool_friendly_identifiers_only() {
        // Arrange
        let valid = ["tasks", "agent.inbox", "order_events-2"];
        let invalid = ["", " ", "has space", "slash/ed", &"x".repeat(101)];

        // Act + Assert
        for name in valid {
            assert_eq!(validate_mailbox_name(name), Ok(()), "{name} must be valid");
        }
        for name in invalid {
            assert!(
                validate_mailbox_name(name).is_err(),
                "{name} must be rejected"
            );
        }
    }

    #[test]
    fn claim_tokens_use_the_mail_prefix_and_are_unique() {
        // Arrange
        let first = new_claim_token();
        let second = new_claim_token();

        // Assert
        assert!(first.starts_with("mlc_"));
        assert_ne!(first, second);
    }
}
