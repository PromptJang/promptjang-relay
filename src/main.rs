mod api;
mod auth;
mod domain;
mod config;
mod models;
mod worker;

use anyhow::Result;
use config::Config;
use reqwest::redirect::Policy;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "promptjang_webhooks=info,tower_http=info".into()),
        )
        .init();
    let config = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    auth::bootstrap_owner(&pool, &config).await?;

    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    tokio::spawn(worker::run(pool.clone(), client));

    let app =
        api::router(api::AppState { pool }, config.static_dir).layer(TraceLayer::new_for_http());
    let listener = TcpListener::bind(&config.bind).await?;
    tracing::info!(bind=%config.bind, "PromptJang Webhooks OSS is ready");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler")
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}
