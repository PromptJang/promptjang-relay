use std::collections::HashMap;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

use crate::{
    domain::models::{ApiKeyView, AttemptView, EndpointView, EventView},
    domain::validation::{MAX_ENDPOINTS, MAX_KEYS, extract_header, validate_name},
};


#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

pub struct AppError(StatusCode, String);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(error: anyhow::Error) -> Self {
        Self(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }
}

impl From<crate::domain::DomainError> for AppError {
    fn from(error: crate::domain::DomainError) -> Self {
        let status = match error.kind {
            crate::domain::ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            crate::domain::ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            crate::domain::ErrorKind::NotFound => StatusCode::NOT_FOUND,
            crate::domain::ErrorKind::Conflict => StatusCode::CONFLICT,
            crate::domain::ErrorKind::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            crate::domain::ErrorKind::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            crate::domain::ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self(status, error.message)
    }
}

type Result<T> = std::result::Result<T, AppError>;

pub fn router(state: AppState, static_dir: String) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/api/login", post(login))
        .route("/api/endpoints", get(list_endpoints).post(create_endpoint))
        .route(
            "/api/endpoints/{id}",
            patch(update_endpoint).delete(delete_endpoint),
        )
        .route("/api/keys", get(list_keys).post(create_key))
        .route("/api/keys/{id}", delete(delete_key))
        .route("/api/events", get(list_events))
        .route("/api/events/{id}", get(get_event))
        .route("/api/events/{id}/replay", post(replay_event))
        .route("/e/{endpoint_id}", post(ingest))
        .fallback_service(
            ServeDir::new(&static_dir).fallback(ServeFile::new(format!("{static_dir}/index.html"))),
        )
        .with_state(state)
}

async fn health() -> impl IntoResponse {
    Json(json!({"status":"ok"}))
}

async fn ready(State(state): State<AppState>) -> Result<impl IntoResponse> {
    sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .map_err(internal)?;
    Ok(Json(json!({"status":"ready"})))
}

#[derive(Deserialize)]
struct LoginInput {
    email: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    Json(input): Json<LoginInput>,
) -> Result<impl IntoResponse> {
    let owner =
        sqlx::query_as::<_, (Uuid, String)>("SELECT id, password_hash FROM owners WHERE email=$1")
            .bind(input.email.to_lowercase())
            .fetch_optional(&state.pool)
            .await
            .map_err(internal)?;
    let Some((owner_id, encoded)) = owner else {
        return Err(unauthorized("invalid credentials"));
    };
    if !crate::domain::secrets::verify_password(&input.password, &encoded) {
        return Err(unauthorized("invalid credentials"));
    }
    let token = crate::domain::secrets::new_secret("pj_session_");
    sqlx::query("INSERT INTO sessions (id,owner_id,token_hash,expires_at) VALUES ($1,$2,$3,$4)")
        .bind(Uuid::new_v4())
        .bind(owner_id)
        .bind(crate::domain::secrets::hash_secret(&token))
        .bind(Utc::now() + Duration::hours(24))
        .execute(&state.pool)
        .await
        .map_err(internal)?;
    Ok(Json(json!({"token":token,"expires_in":86400})))
}

async fn list_endpoints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    let endpoints = crate::store::endpoints::list(&state.pool).await?;
    Ok(Json(json!({"endpoints":endpoints})))
}

#[derive(Deserialize)]
struct EndpointInput {
    name: String,
    url: String,
    enabled: Option<bool>,
}

async fn create_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<EndpointInput>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    validate_name(&input.name)?;
    crate::domain::validation::validate_public_https(&input.url).await?;
    let secret = crate::domain::secrets::new_secret("whsec_");
    let (id, secret) =
        crate::store::endpoints::create(&state.pool, input.name, input.url, secret, input.enabled.unwrap_or(true))
            .await?;
    Ok((StatusCode::CREATED, Json(json!({"id":id,"secret":secret}))))
}

async fn update_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(input): Json<EndpointInput>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    validate_name(&input.name)?;
    crate::domain::validation::validate_public_https(&input.url).await?;
    crate::store::endpoints::update(&state.pool, id, input.name, input.url, input.enabled).await?;
    Ok(Json(json!({"updated":true})))
}

async fn delete_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    crate::store::endpoints::delete(&state.pool, id).await?;
    Ok(Json(json!({"deleted":true})))
}

async fn list_keys(State(state): State<AppState>, headers: HeaderMap) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    let keys = crate::store::keys::list(&state.pool).await?;
    Ok(Json(json!({"keys":keys})))
}

#[derive(Deserialize)]
struct KeyInput {
    name: String,
}
async fn create_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<KeyInput>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    validate_name(&input.name)?;
    let (id, secret) = crate::store::keys::create(&state.pool, input.name).await?;
    Ok((StatusCode::CREATED, Json(json!({"id":id,"key":secret}))))
}

async fn delete_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    crate::store::keys::delete(&state.pool, id).await?;
    Ok(Json(json!({"deleted":true})))
}

async fn list_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    let limit = query
        .get("limit")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(100)
        .clamp(1, 100);
    let events = crate::store::events::list(&state.pool, limit).await?;
    Ok(Json(json!({"events":events})))
}

async fn get_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    let (event, attempts) = crate::store::events::get(&state.pool, id)
        .await?
        .ok_or_else(|| not_found("event not found"))?;
    Ok(Json(json!({"event":event,"attempts":attempts})))
}

async fn replay_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    let replay = crate::store::events::replay(&state.pool, id).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"id":replay,"status":"QUEUED","is_replay":true})),
    ))
}

async fn ingest(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(endpoint_id): Path<Uuid>,
    body: Bytes,
) -> Result<impl IntoResponse> {
    crate::store::auth::require_api_key(&headers, &state.pool)
        .await
        .map_err(|_| unauthorized("invalid API key"))?;
    crate::domain::validation::ensure_payload_size(body.len())?;
    let payload: Value = serde_json::from_slice(&body)
        .map_err(|_| AppError(StatusCode::BAD_REQUEST, "payload must be valid JSON".into()))?;
    let payload_hash = crate::domain::secrets::hash_bytes(&body);
    let idempotency = crate::domain::validation::idempotency_key(&headers)?;
    let key_hash = idempotency.as_deref().map(crate::domain::secrets::hash_secret);
    let event_type = extract_header(&headers, "X-Event-Type");
    let correlation_id = extract_header(&headers, "X-Correlation-ID");
    match crate::store::events::ingest(
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
        crate::store::events::IngestOutcome::Created { id } => Ok((
            StatusCode::ACCEPTED,
            Json(json!({"id":id,"status":"QUEUED"})),
        )),
        crate::store::events::IngestOutcome::IdempotentReplay { id, status } => Ok((
            StatusCode::ACCEPTED,
            Json(json!({"id":id,"status":status,"idempotent_replay":true})),
        )),
    }
}

async fn session(headers: &HeaderMap, pool: &PgPool) -> Result<Uuid> {
    crate::store::auth::require_session(headers, pool)
        .await
        .map_err(|_| unauthorized("invalid or expired session"))
}

fn internal(error: impl std::fmt::Display) -> AppError {
    tracing::error!(%error,"request failed");
    AppError(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal server error".into(),
    )
}
fn unauthorized(message: &str) -> AppError {
    AppError(StatusCode::UNAUTHORIZED, message.into())
}
fn conflict(message: &str) -> AppError {
    AppError(StatusCode::CONFLICT, message.into())
}
fn not_found(message: &str) -> AppError {
    AppError(StatusCode::NOT_FOUND, message.into())
}
