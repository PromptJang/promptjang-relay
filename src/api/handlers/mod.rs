pub mod endpoints;
pub mod events;
pub mod health;
pub mod ingest;
pub mod keys;
pub mod login;
pub mod mail;

use axum::http::HeaderMap;
use sqlx::PgPool;
use uuid::Uuid;

use crate::api::error::AppError;
use crate::store;

pub(crate) async fn session(headers: &HeaderMap, pool: &PgPool) -> Result<Uuid, AppError> {
    store::auth::require_session(headers, pool)
        .await
        .map_err(|_| AppError::unauthorized("invalid or expired session"))
}
