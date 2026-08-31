# PromptJang Relay v0.5.0

Relay v0.5 makes its agent mailbox a production-ready remote MCP service while keeping webhook delivery unchanged.

## Highlights

- Authenticated Streamable HTTP MCP for public HTTPS and private HTTPS/VPN deployments.
- PostgreSQL-backed MCP session recovery across Relay instances without sticky sessions.
- An Integrations screen with configured Codex, Claude Code, OpenCode, and Qwen recipes.
- Strict unrestricted-key, host, origin, and public-URL validation.
- One-time owner bootstrap that no longer revokes sessions when another instance starts.
- Pull-request CI and remote MCP integration coverage against PostgreSQL.

Relay still does not run or wake agents. The user, CLI, or scheduler owns the agent loop.

## Upgrade note

The deprecated `promptjang-relay-mcp` database-connected stdio companion has been removed. Point MCP clients at `https://your-relay.example/mcp` with an unrestricted Relay API key. The Relay server remains the only component that receives `DATABASE_URL`.

Webhook signing remains Standard Webhooks v1, and the mailbox API/tool contract is unchanged.
