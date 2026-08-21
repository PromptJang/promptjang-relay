use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::api::AppState;
use crate::api::error::ApiResult;
use crate::api::handlers::session;
use crate::domain::validation::{validate_name, validate_public_https};
use crate::domain::secrets;
use crate::store;

#[derive(Deserialize)]
pub struct EndpointInput {
    name: String,
    url: String,
    enabled: Option<bool>,
}

pub async fn list_endpoints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    session(&headers, &state.pool).await?;
    let endpoints = store::endpoints::list(&state.pool).await?;
    Ok(Json(json!({"endpoints":endpoints})))
}

pub async fn create_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<EndpointInput>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    session(&headers, &state.pool).await?;
    validate_name(&input.name)?;
    validate_public_https(&input.url).await?;
    let signing_secret = secrets::new_secret("whsec_");
    let (id, secret) = store::endpoints::create(
        &state.pool,
        input.name,
        input.url,
        signing_secret,
        input.enabled.unwrap_or(true),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(json!({"id":id,"secret":secret}))))
}

pub async fn update_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<EndpointInput>,
) -> ApiResult<Json<serde_json::Value>> {
    session(&headers, &state.pool).await?;
    validate_name(&input.name)?;
    validate_public_https(&input.url).await?;
    store::endpoints::update(&state.pool, id, input.name, input.url, input.enabled).await?;
    Ok(Json(json!({"updated":true})))
}

pub async fn delete_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    session(&headers, &state.pool).await?;
    store::endpoints::delete(&state.pool, id).await?;
    Ok(Json(json!({"deleted":true})))
}
