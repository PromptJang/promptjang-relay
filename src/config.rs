use anyhow::{Context, Result};
use base64::Engine;
use ipnet::IpNet;
use std::env;

#[derive(Clone)]
pub struct Config {
    pub database_url: String,
    pub bind: String,
    pub admin_email: Option<String>,
    pub admin_password: Option<String>,
    pub static_dir: String,
    pub encryption_key: [u8; 32],
    pub max_payload_bytes: usize,
    pub rate_limit_per_minute: i64,
    pub retention_days: i64,
    pub worker_concurrency: usize,
    pub delivery_timeout_seconds: u64,
    pub retry_delays_seconds: Vec<i64>,
    pub stuck_after_seconds: i64,
    pub response_body_bytes: usize,
    pub db_max_connections: u32,
    pub session_ttl_seconds: i64,
    pub allow_private_cidrs: Vec<IpNet>,
    pub allow_insecure_http: bool,
    pub allow_weak_password: bool,
    pub extra_ca_cert_path: Option<String>,
    pub otel_enabled: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_reader(|key| env::var(key).ok())
    }

    pub fn from_reader(read: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let database_url = read("DATABASE_URL").context("DATABASE_URL is required")?;
        let encoded_key = read("PJ_ENCRYPTION_KEY")
            .context("PJ_ENCRYPTION_KEY is required and must be a base64-encoded 32-byte key")?;
        let key_bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded_key)
            .context("PJ_ENCRYPTION_KEY must be valid base64")?;
        let encryption_key: [u8; 32] = key_bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("PJ_ENCRYPTION_KEY must decode to exactly 32 bytes"))?;

        let parse = |name: &str, default: &str| -> Result<String> {
            Ok(read(name).unwrap_or_else(|| default.to_string()))
        };
        let retry_delays_seconds = parse("PJ_RETRY_DELAYS_SECONDS", "60,120,240,480,960")?
            .split(',')
            .map(|value| value.trim().parse::<i64>())
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("PJ_RETRY_DELAYS_SECONDS must be comma-separated positive integers")?;
        if retry_delays_seconds.is_empty() || retry_delays_seconds.iter().any(|value| *value <= 0) {
            anyhow::bail!("PJ_RETRY_DELAYS_SECONDS must contain positive values");
        }
        let allow_private_cidrs = read("PJ_DESTINATION_ALLOW_PRIVATE_CIDRS")
            .unwrap_or_default()
            .split(',')
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().parse::<IpNet>())
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("PJ_DESTINATION_ALLOW_PRIVATE_CIDRS must contain valid CIDRs")?;
        let bool_value = |name: &str, default: bool| {
            read(name)
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(default)
        };
        let otel_enabled =
            bool_value("PJ_OTEL_ENABLED", false) && !bool_value("OTEL_SDK_DISABLED", false);
        if otel_enabled
            && read("OTEL_EXPORTER_OTLP_ENDPOINT").is_none_or(|value| value.trim().is_empty())
        {
            anyhow::bail!("OTEL_EXPORTER_OTLP_ENDPOINT is required when PJ_OTEL_ENABLED=true");
        }
        Ok(Self {
            database_url,
            bind: read("PJ_BIND").unwrap_or_else(|| "0.0.0.0:8080".into()),
            admin_email: read("PJ_ADMIN_EMAIL"),
            admin_password: read("PJ_ADMIN_PASSWORD"),
            static_dir: read("PJ_STATIC_DIR").unwrap_or_else(|| "web/dist".into()),
            encryption_key,
            max_payload_bytes: parse("PJ_MAX_PAYLOAD_BYTES", "1048576")?.parse()?,
            rate_limit_per_minute: parse("PJ_RATE_LIMIT_PER_DESTINATION_PER_MINUTE", "10000")?
                .parse()?,
            retention_days: parse("PJ_EVENT_RETENTION_DAYS", "30")?.parse()?,
            worker_concurrency: parse("PJ_WORKER_CONCURRENCY", "8")?.parse()?,
            delivery_timeout_seconds: parse("PJ_DELIVERY_TIMEOUT_SECONDS", "15")?.parse()?,
            retry_delays_seconds,
            stuck_after_seconds: parse("PJ_STUCK_AFTER_SECONDS", "300")?.parse()?,
            response_body_bytes: parse("PJ_RESPONSE_BODY_BYTES", "10240")?.parse()?,
            db_max_connections: parse("PJ_DB_MAX_CONNECTIONS", "20")?.parse()?,
            session_ttl_seconds: parse("PJ_SESSION_TTL_SECONDS", "86400")?.parse()?,
            allow_private_cidrs,
            allow_insecure_http: bool_value("PJ_ALLOW_INSECURE_HTTP", false),
            allow_weak_password: bool_value("PJ_ALLOW_WEAK_PASSWORD", false),
            extra_ca_cert_path: read("PJ_EXTRA_CA_CERT_PATH"),
            otel_enabled,
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
            "PJ_ENCRYPTION_KEY" => Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into()),
            _ => None,
        };

        // Act
        let config = Config::from_reader(reader).expect("valid configuration");

        // Assert
        assert_eq!(config.bind, "0.0.0.0:8080");
        assert_eq!(config.static_dir, "web/dist");
        assert_eq!(config.admin_email, None);
        assert_eq!(config.admin_password, None);
        assert_eq!(config.max_payload_bytes, 1_048_576);
        assert_eq!(config.rate_limit_per_minute, 10_000);
        assert!(!config.otel_enabled);
    }

    #[test]
    fn provided_values_override_defaults() {
        // Arrange
        let reader = |key: &str| match key {
            "DATABASE_URL" => Some("postgres://db/promptjang".into()),
            "PJ_BIND" => Some("127.0.0.1:9000".into()),
            "PJ_ADMIN_EMAIL" => Some("owner@example.com".into()),
            "PJ_ADMIN_PASSWORD" => Some("at-least-twelve".into()),
            "PJ_ENCRYPTION_KEY" => Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into()),
            _ => None,
        };

        // Act
        let config = Config::from_reader(reader).expect("valid configuration");

        // Assert
        assert_eq!(config.bind, "127.0.0.1:9000");
        assert_eq!(config.admin_email.as_deref(), Some("owner@example.com"));
        assert_eq!(config.admin_password.as_deref(), Some("at-least-twelve"));
    }

    #[test]
    fn telemetry_gate_requires_an_explicit_non_empty_endpoint() {
        let reader = |key: &str| match key {
            "DATABASE_URL" => Some("postgres://db/promptjang".into()),
            "PJ_ENCRYPTION_KEY" => Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into()),
            "PJ_OTEL_ENABLED" => Some("true".into()),
            "OTEL_EXPORTER_OTLP_ENDPOINT" => Some(String::new()),
            _ => None,
        };

        let error = Config::from_reader(reader)
            .err()
            .map(|error| error.to_string());

        assert_eq!(
            error.as_deref(),
            Some("OTEL_EXPORTER_OTLP_ENDPOINT is required when PJ_OTEL_ENABLED=true")
        );
    }

    #[test]
    fn sdk_disabled_overrides_the_relay_telemetry_gate() {
        let reader = |key: &str| match key {
            "DATABASE_URL" => Some("postgres://db/promptjang".into()),
            "PJ_ENCRYPTION_KEY" => Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into()),
            "PJ_OTEL_ENABLED" | "OTEL_SDK_DISABLED" => Some("true".into()),
            _ => None,
        };

        let config = Config::from_reader(reader).expect("disabled SDK needs no exporter endpoint");

        assert!(!config.otel_enabled);
    }
}
