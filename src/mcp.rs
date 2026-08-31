use rmcp::ErrorData as McpError;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
    ToolAnnotations,
};
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{ServerHandler, model};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::secrets;
use crate::store::mail::{self, IncomingMessage};

const SERVER_NAME: &str = "promptjang-relay";

#[derive(Clone)]
pub struct RelayMcp {
    pool: PgPool,
}

impl RelayMcp {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn call_mail_tool(&self, name: &str, arguments: Value) -> Result<Value, String> {
        match name {
            "mail_push" => self.mail_push(&arguments).await,
            "mail_claim" => self.mail_claim(&arguments).await,
            "mail_ack" => self.mail_complete(&arguments, true).await,
            "mail_nack" => self.mail_complete(&arguments, false).await,
            "mail_list" => self.mail_list().await,
            other => Err(format!("unknown tool: {other}")),
        }
    }

    fn mailbox(arguments: &Value) -> Result<&str, String> {
        arguments
            .get("mailbox")
            .and_then(Value::as_str)
            .ok_or_else(|| "mailbox is required".to_string())
    }

    async fn mail_push(&self, arguments: &Value) -> Result<Value, String> {
        let name = Self::mailbox(arguments)?;
        mail::validate_mailbox_name(name).map_err(|error| error.to_string())?;
        let payload = arguments
            .get("payload")
            .cloned()
            .ok_or_else(|| "payload is required".to_string())?;
        let (payload_raw, parsed, content_type) = match &payload {
            Value::String(text) => (text.as_bytes().to_vec(), None, "text/plain".to_string()),
            value => (
                serde_json::to_vec(value).map_err(|error| error.to_string())?,
                Some(value.clone()),
                "application/json".to_string(),
            ),
        };
        let payload_sha256 = secrets::hash_bytes(&payload_raw);
        let idempotency_key_hash = arguments
            .get("idempotency_key")
            .and_then(Value::as_str)
            .map(secrets::hash_secret);
        let outcome = mail::push(
            &self.pool,
            name,
            IncomingMessage {
                payload_raw,
                payload: parsed,
                content_type,
                payload_sha256,
                idempotency_key_hash,
                traceparent: None,
                tracestate: None,
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        Ok(match outcome {
            mail::MailPushOutcome::Created { id } => {
                json!({ "id": id, "mailbox": name, "status": "UNREAD" })
            }
            mail::MailPushOutcome::IdempotentReplay { id, status } => json!({
                "id": id,
                "mailbox": name,
                "status": status,
                "idempotent_replay": true
            }),
        })
    }

    async fn mail_claim(&self, arguments: &Value) -> Result<Value, String> {
        let name = Self::mailbox(arguments)?;
        mail::validate_mailbox_name(name).map_err(|error| error.to_string())?;
        let limit = mail::normalize_claim_limit(arguments.get("limit").and_then(Value::as_i64));
        let lease = mail::normalize_lease(arguments.get("lease_seconds").and_then(Value::as_i64));
        let messages = mail::claim(&self.pool, name, limit, lease.seconds)
            .await
            .map_err(|error| error.to_string())?;
        let messages = messages
            .into_iter()
            .map(|message| {
                json!({
                    "id": message.id,
                    "claim_token": message.claim_token,
                    "payload": String::from_utf8_lossy(&message.payload_raw),
                    "payload_json": message.payload,
                    "claim_count": message.claim_count,
                    "created_at": message.created_at
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "mailbox": name, "lease_seconds": lease.seconds, "messages": messages }))
    }

    async fn mail_complete(&self, arguments: &Value, acknowledge: bool) -> Result<Value, String> {
        let name = Self::mailbox(arguments)?;
        mail::validate_mailbox_name(name).map_err(|error| error.to_string())?;
        let id = arguments
            .get("id")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or_else(|| "id must be a message id from mail_claim".to_string())?;
        let claim_token = arguments
            .get("claim_token")
            .and_then(Value::as_str)
            .ok_or_else(|| "claim_token is required".to_string())?;
        let changed = mail::acknowledge(&self.pool, name, id, claim_token, acknowledge)
            .await
            .map_err(|error| error.to_string())?;
        if !changed {
            return Err(
                "message is not claimed with this token (already completed or lease expired)"
                    .to_string(),
            );
        }
        Ok(json!({
            "id": id,
            "mailbox": name,
            "status": if acknowledge { "ACKNOWLEDGED" } else { "UNREAD" }
        }))
    }

    async fn mail_list(&self) -> Result<Value, String> {
        let mailboxes = mail::list_mailboxes(&self.pool)
            .await
            .map_err(|error| error.to_string())?;
        Ok(json!({ "mailboxes": mailboxes }))
    }
}

impl ServerHandler for RelayMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder().enable_tools().build(),
        )
        .with_protocol_version(ProtocolVersion::V_2025_06_18)
        .with_server_info(Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION")))
        .with_instructions(
            "Durable agent mailbox tools. Relay stores messages but does not wake or loop agents.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(tool_definitions()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let arguments = Value::Object(request.arguments.unwrap_or_default());
        match self.call_mail_tool(request.name.as_ref(), arguments).await {
            Ok(value) => Ok(CallToolResult::structured(value).into()),
            Err(message) => Ok(CallToolResult::error(vec![ContentBlock::text(message)]).into()),
        }
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        tool_definitions()
            .into_iter()
            .find(|tool| tool.name == name)
    }
}

fn tool(
    name: &'static str,
    description: &'static str,
    input_schema: Value,
    annotations: ToolAnnotations,
) -> Tool {
    Tool::new(name, description, model::object(input_schema)).annotate(annotations)
}

pub fn tool_definitions() -> Vec<Tool> {
    let mailbox = json!({
        "type": "string",
        "description": "Mailbox name",
        "minLength": 1,
        "maxLength": 100
    });
    vec![
        tool(
            "mail_push",
            "Push a durable message into a PromptJang Relay mailbox.",
            json!({
                "type": "object",
                "properties": {
                    "mailbox": mailbox,
                    "payload": { "description": "JSON value or text message" },
                    "idempotency_key": { "type": "string", "description": "Optional producer deduplication key" }
                },
                "required": ["mailbox", "payload"]
            }),
            ToolAnnotations::new()
                .read_only(false)
                .destructive(false)
                .idempotent(false)
                .open_world(false),
        ),
        tool(
            "mail_claim",
            "Claim pending messages for a bounded lease. Process them, then acknowledge or requeue them.",
            json!({
                "type": "object",
                "properties": {
                    "mailbox": mailbox,
                    "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 10 },
                    "lease_seconds": { "type": "integer", "minimum": 30, "maximum": 3600, "default": 300 }
                },
                "required": ["mailbox"]
            }),
            ToolAnnotations::new()
                .read_only(false)
                .destructive(false)
                .idempotent(false)
                .open_world(false),
        ),
        tool(
            "mail_ack",
            "Acknowledge a successfully processed claimed message.",
            json!({
                "type": "object",
                "properties": {
                    "mailbox": mailbox,
                    "id": { "type": "string", "format": "uuid" },
                    "claim_token": { "type": "string" }
                },
                "required": ["mailbox", "id", "claim_token"]
            }),
            ToolAnnotations::new()
                .read_only(false)
                .destructive(true)
                .idempotent(false)
                .open_world(false),
        ),
        tool(
            "mail_nack",
            "Return a claimed message to UNREAD after processing fails.",
            json!({
                "type": "object",
                "properties": {
                    "mailbox": mailbox,
                    "id": { "type": "string", "format": "uuid" },
                    "claim_token": { "type": "string" }
                },
                "required": ["mailbox", "id", "claim_token"]
            }),
            ToolAnnotations::new()
                .read_only(false)
                .destructive(false)
                .idempotent(false)
                .open_world(false),
        ),
        tool(
            "mail_list",
            "List mailboxes and their unread, claimed, and acknowledged counts.",
            json!({ "type": "object", "properties": {} }),
            ToolAnnotations::new()
                .read_only(true)
                .destructive(false)
                .idempotent(true)
                .open_world(false),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_use_one_relay_mailbox_contract() {
        let tools = tool_definitions();
        let names = tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "mail_push",
                "mail_claim",
                "mail_ack",
                "mail_nack",
                "mail_list"
            ]
        );
        for tool in tools.iter().take(4) {
            assert_eq!(
                tool.input_schema
                    .get("required")
                    .and_then(Value::as_array)
                    .and_then(|required| required.first())
                    .and_then(Value::as_str),
                Some("mailbox")
            );
        }
        assert_eq!(
            tools[4].annotations.as_ref().and_then(|a| a.read_only_hint),
            Some(true)
        );
    }

    #[test]
    fn tool_schemas_match_the_agent_mailbox_fixture() {
        let fixture: Value =
            serde_json::from_str(include_str!("../tests/fixtures/agent-mailbox-v1.json"))
                .expect("agent mailbox fixture must be valid JSON");
        let actual = Value::Array(
            tool_definitions()
                .into_iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "inputSchema": tool.input_schema
                    })
                })
                .collect(),
        );
        assert_eq!(actual, fixture["tools"]);
        assert_eq!(
            fixture["states"],
            json!(["UNREAD", "CLAIMED", "ACKNOWLEDGED"])
        );
    }

    #[test]
    fn string_payload_hashes_the_exact_stored_bytes() {
        let raw = b"line one\nline two";
        assert_eq!(secrets::hash_bytes(raw), secrets::hash_bytes(raw));
        assert_ne!(
            secrets::hash_bytes(raw),
            secrets::hash_bytes(b"line one line two")
        );
    }
}
