use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::api::AppState;
use crate::api::error::{ApiResult, AppError};
use crate::domain::secrets;
use crate::domain::validation::{ensure_payload_size, extract_header, idempotency_key};
use crate::store;
use crate::store::events::IngestOutcome;

pub async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(endpoint_id): Path<Uuid>,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    store::auth::require_api_key(&headers, &state.pool)
        .await
        .map_err(|_| AppError::unauthorized("invalid API key"))?;
    ensure_payload_size(body.len())?;
    let payload: serde_json::Value = serde_json::from_slice(&body).map_err(|_| {
        AppError::from(crate::domain::DomainError::bad_request(
            "payload must be valid JSON",
        ))
    })?;
    let payload_hash = secrets::hash_bytes(&body);
    let idempotency = idempotency_key(&headers)?;
    let key_hash = idempotency.as_deref().map(secrets::hash_secret);
    let event_type = extract_header(&headers, "X-Event-Type");
    let correlation_id = extract_header(&headers, "X-Correlation-ID");
    match store::events::ingest(
        &state.pool,
        endpoint_id,
        payload,
        payload_hash,
        key_hash,
        event_type,
        correlation_id,
    )
    .await?
    {
        IngestOutcome::Created { id } => Ok((
            StatusCode::ACCEPTED,
            Json(json!({"id":id,"status":"QUEUED"})),
        )),
        IngestOutcome::IdempotentReplay { id, status } => Ok((
            StatusCode::ACCEPTED,
            Json(json!({"id":id,"status":status,"idempotent_replay":true})),
        )),
    }
}
