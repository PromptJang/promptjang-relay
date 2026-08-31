use rmcp::transport::streamable_http_server::session::store::{
    SessionState, SessionStore, SessionStoreError,
};
use sqlx::PgPool;

#[derive(Clone, Debug)]
pub struct PostgresMcpSessionStore {
    pool: PgPool,
    ttl_seconds: i64,
}

impl PostgresMcpSessionStore {
    pub fn new(pool: PgPool, ttl_seconds: i64) -> Self {
        Self { pool, ttl_seconds }
    }
}

#[async_trait::async_trait]
impl SessionStore for PostgresMcpSessionStore {
    async fn load(&self, session_id: &str) -> Result<Option<SessionState>, SessionStoreError> {
        let value = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT initialize_state FROM mcp_sessions WHERE session_id=$1 AND expires_at>now()",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| Box::new(error) as SessionStoreError)?;
        value
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| Box::new(error) as SessionStoreError)
    }

    async fn store(&self, session_id: &str, state: &SessionState) -> Result<(), SessionStoreError> {
        let state =
            serde_json::to_value(state).map_err(|error| Box::new(error) as SessionStoreError)?;
        sqlx::query(
            "INSERT INTO mcp_sessions(session_id,initialize_state,expires_at)
             VALUES($1,$2,now()+make_interval(secs=>$3))
             ON CONFLICT(session_id) DO UPDATE SET initialize_state=$2,
             expires_at=now()+make_interval(secs=>$3)",
        )
        .bind(session_id)
        .bind(state)
        .bind(self.ttl_seconds as f64)
        .execute(&self.pool)
        .await
        .map_err(|error| Box::new(error) as SessionStoreError)?;
        Ok(())
    }

    async fn delete(&self, session_id: &str) -> Result<(), SessionStoreError> {
        sqlx::query("DELETE FROM mcp_sessions WHERE session_id=$1")
            .bind(session_id)
            .execute(&self.pool)
            .await
            .map_err(|error| Box::new(error) as SessionStoreError)?;
        Ok(())
    }
}

pub async fn cleanup(pool: &PgPool) -> anyhow::Result<u64> {
    Ok(
        sqlx::query("DELETE FROM mcp_sessions WHERE expires_at<=now()")
            .execute(pool)
            .await?
            .rows_affected(),
    )
}
