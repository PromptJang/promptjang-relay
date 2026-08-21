mod delivery;

use std::time::Duration;

use reqwest::Client;
use sqlx::PgPool;

pub async fn run(pool: PgPool, client: Client) {
    loop {
        if let Err(error) = delivery::recover_stuck(&pool).await {
            tracing::error!(%error, "stuck delivery recovery failed");
        }
        match delivery::process_one(&pool, &client).await {
            Ok(true) => continue,
            Ok(false) => tokio::time::sleep(Duration::from_millis(500)).await,
            Err(error) => {
                tracing::error!(%error, "delivery worker iteration failed");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn worker_module_exposes_no_panicking_paths() {
        // Arrange
        let source = include_str!("delivery.rs");

        // Act
        let production_panics = source
            .lines()
            .filter(|line| {
                !line.trim_start().starts_with("//")
                    && (line.contains(".unwrap()")
                        || line.contains(".expect(")
                        || line.contains("panic!"))
            })
            .count();

        // Assert
        assert_eq!(production_panics, 0);
    }
}
