use std::collections::HashMap;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::api::AppState;
use crate::api::error::{ApiResult, AppError};
use crate::api::handlers::session;
use crate::store;

pub async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<serde_json::Value>> {
    session(&headers, &state.pool).await?;
    let limit = query
        .get("limit")
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(100)
        .clamp(1, 100);
    let events = store::events::list(&state.pool, limit).await?;
    Ok(Json(json!({"events":events})))
}

pub async fn get_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    session(&headers, &state.pool).await?;
    let (event, attempts) = store::events::get(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::from(crate::domain::DomainError::not_found("event not found")))?;
    Ok(Json(json!({"event":event,"attempts":attempts})))
}

pub async fn replay_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    session(&headers, &state.pool).await?;
    let replay = store::events::replay(&state.pool, id).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"id":replay,"status":"QUEUED","is_replay":true})),
    ))
}
