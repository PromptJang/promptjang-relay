use std::{collections::HashMap, net::IpAddr};

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
use url::Url;
use uuid::Uuid;

use crate::{
    auth,
    domain::models::{ApiKeyView, AttemptView, EndpointView, EventView},
    domain::validation::{
        MAX_ENDPOINTS, MAX_KEYS, MAX_PAYLOAD_BYTES, extract_header, validate_name,
    },
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
    if !auth::verify_password(&input.password, &encoded) {
        return Err(unauthorized("invalid credentials"));
    }
    let token = auth::new_secret("pj_session_");
    sqlx::query("INSERT INTO sessions (id,owner_id,token_hash,expires_at) VALUES ($1,$2,$3,$4)")
        .bind(Uuid::new_v4())
        .bind(owner_id)
        .bind(auth::hash_secret(&token))
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
    let endpoints = sqlx::query_as::<_, EndpointView>(
        "SELECT id,name,url,enabled,created_at,updated_at FROM endpoints ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(internal)?;
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
    validate_public_https(&input.url).await?;
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM endpoints")
        .fetch_one(&state.pool)
        .await
        .map_err(internal)?;
    if count >= MAX_ENDPOINTS {
        return Err(conflict("endpoint limit reached (10)"));
    }
    let id = Uuid::new_v4();
    let secret = auth::new_secret("whsec_");
    sqlx::query(
        "INSERT INTO endpoints (id,name,url,signing_secret,enabled) VALUES ($1,$2,$3,$4,$5)",
    )
    .bind(id)
    .bind(input.name)
    .bind(input.url)
    .bind(&secret)
    .bind(input.enabled.unwrap_or(true))
    .execute(&state.pool)
    .await
    .map_err(internal)?;
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
    validate_public_https(&input.url).await?;
    let changed=sqlx::query("UPDATE endpoints SET name=$2,url=$3,enabled=COALESCE($4,enabled),updated_at=now() WHERE id=$1")
        .bind(id).bind(input.name).bind(input.url).bind(input.enabled).execute(&state.pool).await.map_err(internal)?.rows_affected();
    if changed == 0 {
        return Err(not_found("endpoint not found"));
    }
    Ok(Json(json!({"updated":true})))
}

async fn delete_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    let changed = sqlx::query("DELETE FROM endpoints WHERE id=$1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(internal)?
        .rows_affected();
    if changed == 0 {
        return Err(not_found("endpoint not found"));
    }
    Ok(Json(json!({"deleted":true})))
}

async fn list_keys(State(state): State<AppState>, headers: HeaderMap) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    let keys = sqlx::query_as::<_, ApiKeyView>(
        "SELECT id,name,prefix,last_used_at,created_at FROM api_keys ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(internal)?;
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
    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM api_keys")
        .fetch_one(&state.pool)
        .await
        .map_err(internal)?;
    if count >= MAX_KEYS {
        return Err(conflict("API key limit reached (5)"));
    }
    let id = Uuid::new_v4();
    let secret = auth::new_secret("pj_oss_");
    let prefix = secret.chars().take(15).collect::<String>();
    sqlx::query("INSERT INTO api_keys (id,name,prefix,secret_hash) VALUES ($1,$2,$3,$4)")
        .bind(id)
        .bind(input.name)
        .bind(prefix)
        .bind(auth::hash_secret(&secret))
        .execute(&state.pool)
        .await
        .map_err(internal)?;
    Ok((StatusCode::CREATED, Json(json!({"id":id,"key":secret}))))
}

async fn delete_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    sqlx::query("DELETE FROM api_keys WHERE id=$1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(internal)?;
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
    let events=sqlx::query_as::<_,EventView>("SELECT id,endpoint_id,status,event_type,correlation_id,payload,retry_count,max_retries,is_replay,source_event_id,next_attempt_at,last_error,created_at,updated_at FROM events ORDER BY created_at DESC LIMIT $1")
        .bind(limit).fetch_all(&state.pool).await.map_err(internal)?;
    Ok(Json(json!({"events":events})))
}

async fn get_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    let event=sqlx::query_as::<_,EventView>("SELECT id,endpoint_id,status,event_type,correlation_id,payload,retry_count,max_retries,is_replay,source_event_id,next_attempt_at,last_error,created_at,updated_at FROM events WHERE id=$1")
        .bind(id).fetch_optional(&state.pool).await.map_err(internal)?.ok_or_else(||not_found("event not found"))?;
    let attempts=sqlx::query_as::<_,AttemptView>("SELECT id,event_id,status_code,response_body,duration_ms,error,attempted_at FROM delivery_attempts WHERE event_id=$1 ORDER BY attempted_at")
        .bind(id).fetch_all(&state.pool).await.map_err(internal)?;
    Ok(Json(json!({"event":event,"attempts":attempts})))
}

async fn replay_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse> {
    session(&headers, &state.pool).await?;
    let source=sqlx::query_as::<_,(Uuid,Value,String,Option<String>,Option<String>)>("SELECT endpoint_id,payload,payload_sha256,event_type,correlation_id FROM events WHERE id=$1")
        .bind(id).fetch_optional(&state.pool).await.map_err(internal)?.ok_or_else(||not_found("event not found"))?;
    let replay = Uuid::new_v4();
    sqlx::query("INSERT INTO events (id,endpoint_id,status,payload,payload_sha256,event_type,correlation_id,is_replay,source_event_id) VALUES ($1,$2,'QUEUED',$3,$4,$5,$6,true,$7)")
        .bind(replay).bind(source.0).bind(source.1).bind(source.2).bind(source.3).bind(source.4).bind(id).execute(&state.pool).await.map_err(internal)?;
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
    auth::require_api_key(&headers, &state.pool)
        .await
        .map_err(|_| unauthorized("invalid API key"))?;
    if body.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "payload exceeds 256 KB".into(),
        ));
    }
    let payload: Value = serde_json::from_slice(&body)
        .map_err(|_| AppError(StatusCode::BAD_REQUEST, "payload must be valid JSON".into()))?;
    let enabled = sqlx::query_scalar::<_, bool>("SELECT enabled FROM endpoints WHERE id=$1")
        .bind(endpoint_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(internal)?
        .ok_or_else(|| not_found("endpoint not found"))?;
    if !enabled {
        return Err(conflict("endpoint is disabled"));
    }
    let minute_count=sqlx::query_scalar::<_,i64>("SELECT count(*) FROM events WHERE endpoint_id=$1 AND is_replay=false AND created_at>=date_trunc('minute',now())")
        .bind(endpoint_id).fetch_one(&state.pool).await.map_err(internal)?;
    if minute_count >= 1000 {
        return Err(AppError(
            StatusCode::TOO_MANY_REQUESTS,
            "accepted-event limit reached".into(),
        ));
    }
    let payload_hash = hex::encode(Sha256::digest(&body));
    let idempotency = headers.get("Idempotency-Key").and_then(|v| v.to_str().ok());
    if idempotency.is_some_and(|value| value.is_empty() || value.len() > 255) {
        return Err(AppError(
            StatusCode::BAD_REQUEST,
            "Idempotency-Key must contain 1 to 255 characters".into(),
        ));
    }
    let key_hash = idempotency.map(auth::hash_secret);
    let mut tx = state.pool.begin().await.map_err(internal)?;
    if let Some(ref hash) = key_hash {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("{endpoint_id}:{hash}"))
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
        if let Some((existing_id,existing_hash,status))=sqlx::query_as::<_,(Uuid,String,String)>("SELECT id,payload_sha256,status FROM events WHERE endpoint_id=$1 AND idempotency_key_hash=$2 AND is_replay=false")
            .bind(endpoint_id).bind(hash).fetch_optional(&mut *tx).await.map_err(internal)?{
            if existing_hash!=payload_hash{return Err(conflict("Idempotency-Key was already used with a different payload"))}
            tx.commit().await.map_err(internal)?;return Ok((StatusCode::ACCEPTED,Json(json!({"id":existing_id,"status":status,"idempotent_replay":true}))))
        }
    }
    let id = Uuid::new_v4();
    let event_type = extract_header(&headers, "X-Event-Type");
    let correlation_id = extract_header(&headers, "X-Correlation-ID");
    sqlx::query("INSERT INTO events (id,endpoint_id,status,event_type,correlation_id,payload,payload_sha256,idempotency_key_hash) VALUES ($1,$2,'QUEUED',$3,$4,$5,$6,$7)")
        .bind(id).bind(endpoint_id).bind(event_type).bind(correlation_id).bind(payload).bind(payload_hash).bind(key_hash).execute(&mut *tx).await.map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"id":id,"status":"QUEUED"})),
    ))
}

async fn session(headers: &HeaderMap, pool: &PgPool) -> Result<Uuid> {
    auth::require_session(headers, pool)
        .await
        .map_err(|_| unauthorized("invalid or expired session"))
}

async fn validate_public_https(raw: &str) -> Result<()> {
    let url = Url::parse(raw)
        .map_err(|_| AppError(StatusCode::BAD_REQUEST, "invalid endpoint URL".into()))?;
    if url.scheme() != "https" || url.username() != "" || url.password().is_some() {
        return Err(AppError(
            StatusCode::BAD_REQUEST,
            "endpoint must be a public HTTPS URL without credentials".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| AppError(StatusCode::BAD_REQUEST, "endpoint host is required".into()))?;
    if host.eq_ignore_ascii_case("localhost") {
        return Err(AppError(
            StatusCode::BAD_REQUEST,
            "private endpoint hosts are not allowed".into(),
        ));
    }
    let port = url.port_or_known_default().unwrap_or(443);
    let addresses = tokio::net::lookup_host((host, port)).await.map_err(|_| {
        AppError(
            StatusCode::BAD_REQUEST,
            "endpoint host could not be resolved".into(),
        )
    })?;
    if addresses
        .into_iter()
        .any(|address| !public_ip(address.ip()))
    {
        return Err(AppError(
            StatusCode::BAD_REQUEST,
            "private endpoint addresses are not allowed".into(),
        ));
    }
    Ok(())
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            !(v.is_private()
                || v.is_loopback()
                || v.is_link_local()
                || v.is_broadcast()
                || v.is_documentation()
                || v.is_multicast()
                || v.is_unspecified())
        }
        IpAddr::V6(v) => {
            !(v.is_loopback()
                || v.is_unspecified()
                || v.is_unique_local()
                || v.is_unicast_link_local()
                || v.is_multicast())
        }
    }
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
