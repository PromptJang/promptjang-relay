use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct EndpointView {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ApiKeyView {
    pub id: Uuid,
    pub name: String,
    pub prefix: String,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct EventView {
    pub id: Uuid,
    pub endpoint_id: Uuid,
    pub status: String,
    pub event_type: Option<String>,
    pub correlation_id: Option<String>,
    pub payload: serde_json::Value,
    pub retry_count: i32,
    pub max_retries: i32,
    pub is_replay: bool,
    pub source_event_id: Option<Uuid>,
    pub next_attempt_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct AttemptView {
    pub id: Uuid,
    pub event_id: Uuid,
    pub status_code: Option<i32>,
    pub response_body: Option<String>,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub attempted_at: DateTime<Utc>,
}
