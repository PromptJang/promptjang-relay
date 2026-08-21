mod error;
mod handlers;
mod state;

pub use state::AppState;

use axum::Router;
use axum::routing::{delete, get, patch, post};
use tower_http::services::{ServeDir, ServeFile};

use crate::api::handlers::{
    endpoints::{create_endpoint, delete_endpoint, list_endpoints, update_endpoint},
    events::{get_event, list_events, replay_event},
    health::{health, ready},
    ingest::ingest,
    keys::{create_key, delete_key, list_keys},
    login::login,
};

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
