use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use serde_json::json;

use crate::api::AppState;
use crate::api::error::{ApiResult, AppError};
use crate::store;

#[derive(Deserialize)]
pub struct LoginInput {
    email: String,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let owner_id = store::auth::verify_login(&state.pool, &input.email, &input.password)
        .await
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::unauthorized("invalid credentials"))?;
    let token = store::auth::issue_session(&state.pool, owner_id)
        .await
        .map_err(AppError::from)?;
    Ok(Json(json!({"token":token,"expires_in":86400})))
}
