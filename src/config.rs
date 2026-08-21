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
        Ok(Self {
            database_url: env::var("DATABASE_URL").context("DATABASE_URL is required")?,
            bind: env::var("PJ_BIND").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            admin_email: env::var("PJ_ADMIN_EMAIL").ok(),
            admin_password: env::var("PJ_ADMIN_PASSWORD").ok(),
            static_dir: env::var("PJ_STATIC_DIR").unwrap_or_else(|_| "web/dist".into()),
        })
    }
}
