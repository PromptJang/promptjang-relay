use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use promptjang_relay::{api, config::Config, domain::secrets, store};
use tower::ServiceExt;

fn config(database_url: &str) -> Arc<Config> {
    Arc::new(
        Config::from_reader(|key| match key {
            "DATABASE_URL" => Some(database_url.into()),
            "PJ_ENCRYPTION_KEY" => Some("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into()),
            _ => None,
        })
        .expect("valid integration configuration"),
    )
}

fn mcp_request(api_key: &str, body: &str) -> Request<Body> {
    Request::post("/mcp")
        .header(header::HOST, "localhost:8080")
        .header(header::AUTHORIZATION, format!("Bearer {api_key}"))
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json, text/event-stream")
        .body(Body::from(body.to_owned()))
        .expect("valid MCP request")
}

#[sqlx::test(migrations = "./migrations")]
#[ignore = "requires a PostgreSQL test role with database creation permission"]
async fn mcp_auth_and_session_restore_across_instances(pool: sqlx::PgPool) {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL provided by CI");
    let config = config(&database_url);
    let (_, unrestricted_key) =
        store::keys::create(&pool, &config.encryption_key, "remote MCP".into(), vec![])
            .await
            .expect("create unrestricted key");

    let missing = Request::get("/mcp")
        .header(header::HOST, "localhost:8080")
        .body(Body::empty())
        .expect("valid request");
    let response = api::router(
        api::AppState {
            pool: pool.clone(),
            config: config.clone(),
        },
        "web/dist".into(),
    )
    .oneshot(missing)
    .await
    .expect("missing-key response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let (revoked_id, revoked_key) =
        store::keys::create(&pool, &config.encryption_key, "revoked MCP".into(), vec![])
            .await
            .expect("create key to revoke");
    store::keys::delete(&pool, revoked_id)
        .await
        .expect("revoke key");
    let revoked = Request::get("/mcp")
        .header(header::HOST, "localhost:8080")
        .header(header::AUTHORIZATION, format!("Bearer {revoked_key}"))
        .body(Body::empty())
        .expect("valid request");
    let response = api::router(
        api::AppState {
            pool: pool.clone(),
            config: config.clone(),
        },
        "web/dist".into(),
    )
    .oneshot(revoked)
    .await
    .expect("revoked-key response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let (destination_id, _) = store::endpoints::create(
        &pool,
        &config.encryption_key,
        "scope".into(),
        "https://example.com/webhook".into(),
        "whsec_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=".into(),
        false,
    )
    .await
    .expect("create destination");
    let (_, scoped_key) = store::keys::create(
        &pool,
        &config.encryption_key,
        "scoped".into(),
        vec![destination_id],
    )
    .await
    .expect("create scoped key");
    let scoped = Request::get("/mcp")
        .header(header::HOST, "localhost:8080")
        .header(header::AUTHORIZATION, format!("Bearer {scoped_key}"))
        .body(Body::empty())
        .expect("valid request");
    let response = api::router(
        api::AppState {
            pool: pool.clone(),
            config: config.clone(),
        },
        "web/dist".into(),
    )
    .oneshot(scoped)
    .await
    .expect("scoped-key response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let initialize = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"relay-test","version":"1"}}}"#;
    let first_instance = api::router(
        api::AppState {
            pool: pool.clone(),
            config: config.clone(),
        },
        "web/dist".into(),
    );
    let response = first_instance
        .oneshot(mcp_request(&unrestricted_key, initialize))
        .await
        .expect("initialize response");
    assert_eq!(response.status(), StatusCode::OK);
    let session_id = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .expect("session id")
        .to_owned();

    let tools = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
    let mut request = mcp_request(&unrestricted_key, tools);
    request.headers_mut().insert(
        "mcp-session-id",
        session_id.parse().expect("valid session header"),
    );
    request.headers_mut().insert(
        "mcp-protocol-version",
        "2025-06-18".parse().expect("valid protocol header"),
    );
    let second_instance = api::router(
        api::AppState {
            pool: pool.clone(),
            config,
        },
        "web/dist".into(),
    );
    let response = second_instance
        .oneshot(request)
        .await
        .expect("restored-session response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 256 * 1024)
        .await
        .expect("MCP body");
    let body = String::from_utf8(body.to_vec()).expect("UTF-8 MCP body");
    for tool in [
        "mail_push",
        "mail_claim",
        "mail_ack",
        "mail_nack",
        "mail_list",
    ] {
        assert!(body.contains(tool), "missing {tool} in {body}");
    }

    let payload_raw = br#"{"task":"survive session cleanup"}"#.to_vec();
    let message = store::mail::push(
        &pool,
        "session-cleanup",
        store::mail::IncomingMessage {
            payload_sha256: secrets::hash_bytes(&payload_raw),
            payload_raw,
            payload: Some(serde_json::json!({"task":"survive session cleanup"})),
            content_type: "application/json".into(),
            idempotency_key_hash: None,
            traceparent: None,
            tracestate: None,
        },
    )
    .await
    .expect("store mailbox message");
    let message_id = match message {
        store::mail::MailPushOutcome::Created { id } => id,
        store::mail::MailPushOutcome::IdempotentReplay { .. } => {
            panic!("new mailbox message must not be an idempotent replay")
        }
    };
    sqlx::query("UPDATE mcp_sessions SET expires_at=now()-interval '1 second' WHERE session_id=$1")
        .bind(&session_id)
        .execute(&pool)
        .await
        .expect("expire session");
    assert_eq!(
        store::mcp_sessions::cleanup(&pool)
            .await
            .expect("clean expired sessions"),
        1
    );
    let message_survived: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM mailbox_messages WHERE id=$1)")
            .bind(message_id)
            .fetch_one(&pool)
            .await
            .expect("query mailbox message");
    assert!(message_survived);
}
