use axum::Json;
use axum::extract::State;
use serde_json::json;

use crate::api::AppState;
use crate::api::error::{ApiResult, AppError};

pub async fn health() -> Json<serde_json::Value> {
    Json(json!({"status":"ok"}))
}

pub async fn ready(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(|error| AppError::from(anyhow::anyhow!(error.to_string())))?;
    Ok(Json(json!({"status":"ready"})))
}
