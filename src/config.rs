use anyhow::{Context, Result};
use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind: String,
    pub admin_email: Option<String>,
    pub admin_password: Option<String>,
    pub static_dir: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_reader(|key| env::var(key).ok())
    }

    pub fn from_reader(read: impl Fn(&str) -> Option<String>) -> Result<Self> {
        Ok(Self {
            database_url: read("DATABASE_URL").context("DATABASE_URL is required")?,
            bind: read("PJ_BIND").unwrap_or_else(|| "0.0.0.0:8080".into()),
            admin_email: read("PJ_ADMIN_EMAIL"),
            admin_password: read("PJ_ADMIN_PASSWORD"),
            static_dir: read("PJ_STATIC_DIR").unwrap_or_else(|| "web/dist".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_database_url_is_rejected() {
        // Arrange
        let reader = |_key: &str| None;

        // Act
        let result = Config::from_reader(reader);

        // Assert
        let message = match result {
            Err(error) => error.to_string(),
            Ok(_) => String::new(),
        };
        assert!(message.contains("DATABASE_URL is required"));
    }

    #[test]
    fn defaults_apply_when_optional_values_are_absent() {
        // Arrange
        let reader = |key: &str| match key {
            "DATABASE_URL" => Some("postgres://localhost/promptjang".into()),
            _ => None,
        };

        // Act
        let config = Config::from_reader(reader).expect("valid configuration");

        // Assert
        assert_eq!(config.bind, "0.0.0.0:8080");
        assert_eq!(config.static_dir, "web/dist");
        assert_eq!(config.admin_email, None);
        assert_eq!(config.admin_password, None);
    }

    #[test]
    fn provided_values_override_defaults() {
        // Arrange
        let reader = |key: &str| match key {
            "DATABASE_URL" => Some("postgres://db/promptjang".into()),
            "PJ_BIND" => Some("127.0.0.1:9000".into()),
            "PJ_ADMIN_EMAIL" => Some("owner@example.com".into()),
            "PJ_ADMIN_PASSWORD" => Some("at-least-twelve".into()),
            _ => None,
        };

        // Act
        let config = Config::from_reader(reader).expect("valid configuration");

        // Assert
        assert_eq!(config.bind, "127.0.0.1:9000");
        assert_eq!(config.admin_email.as_deref(), Some("owner@example.com"));
        assert_eq!(
            config.admin_password.as_deref(),
            Some("at-least-twelve")
        );
    }
}
