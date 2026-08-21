mod api;
mod config;
mod domain;
mod store;
mod telemetry;
mod worker;

use anyhow::Result;
use config::Config;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Arc::new(Config::from_env()?);
    let telemetry = telemetry::init(&config)?;
    let pool = PgPoolOptions::new()
        .max_connections(config.db_max_connections)
        .connect(&config.database_url)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    store::auth::bootstrap_owner(&pool, &config).await?;
    let migrated =
        store::endpoints::migrate_plaintext_secrets(&pool, &config.encryption_key).await?;
    if migrated > 0 {
        tracing::info!(migrated, "encrypted legacy destination secrets");
    }

    let worker_shutdown = CancellationToken::new();
    let worker_task = tokio::spawn(worker::run(
        pool.clone(),
        config.clone(),
        worker_shutdown.clone(),
    ));

    let app = api::router(
        api::AppState {
            pool,
            config: config.clone(),
        },
        config.static_dir.clone(),
    )
    .layer(TraceLayer::new_for_http());
    let listener = TcpListener::bind(&config.bind).await?;
    tracing::info!(bind=%config.bind, "PromptJang Relay is ready");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    worker_shutdown.cancel();
    if tokio::time::timeout(std::time::Duration::from_secs(20), worker_task)
        .await
        .is_err()
    {
        tracing::warn!("delivery workers did not stop within 20 seconds");
    }
    telemetry.shutdown().await;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "Ctrl+C handler failed");
        }
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(error) => {
                tracing::warn!(%error, "SIGTERM handler failed");
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}
