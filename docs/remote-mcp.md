# Remote MCP

Relay serves the agent mailbox through authenticated Streamable HTTP. PostgreSQL stays behind Relay.

```text
CLI agent ── HTTPS + Bearer API key ──▶ Relay /mcp ──▶ PostgreSQL mailbox
```

Every MCP tool takes an explicit mailbox name. Use one retrievable unrestricted `pj_relay_` API key per agent or trust boundary.

## Local development

The default endpoint is `http://localhost:8080/mcp`. Plain HTTP is accepted only for loopback MCP URLs.

```bash
export PJ_RELAY_API_KEY='pj_relay_YOUR_KEY'
codex mcp add promptjang-relay \
  --url http://localhost:8080/mcp \
  --bearer-token-env-var PJ_RELAY_API_KEY
```

The dashboard **Integrations** screen generates tested setup commands for Codex, Claude Code, OpenCode, and Qwen.

## Private network or public hostname

Set Relay's external URL and terminate TLS at a reverse proxy:

```env
PJ_MCP_ENABLED=true
PJ_MCP_PUBLIC_URL=https://relay.example.com/mcp
PJ_MCP_SESSION_TTL_SECONDS=86400
```

The same rule applies to a private DNS name reached through a LAN or VPN: use HTTPS. Do not publish Relay's port 8080 directly to the internet.

An nginx MCP route needs to preserve the public host and bearer header and must not buffer the stream:

```nginx
location /mcp {
    proxy_pass http://relay:8080;
    proxy_http_version 1.1;
    proxy_set_header Host $host;
    proxy_set_header Authorization $http_authorization;
    proxy_buffering off;
    proxy_read_timeout 1h;
    proxy_send_timeout 1h;
}
```

Keep the dashboard and administration API behind the same TLS and network access controls. MCP session initialization is stored in PostgreSQL, so a load balancer does not require sticky sessions.

## Client configuration

| Client | Remote transport |
|---|---|
| Codex | `codex mcp add … --url URL --bearer-token-env-var PJ_RELAY_API_KEY` |
| Claude Code | `claude mcp add --transport http … --header 'Authorization: Bearer …'` |
| OpenCode | `opencode mcp add … --url URL --header 'Authorization=Bearer …'` |
| Qwen Code | `qwen mcp add … URL -t http -H 'Authorization: Bearer …'` |

Codex references an environment variable. The other listed commands store the bearer header in their MCP configuration. Protect those files as credentials and revoke the Relay key if a client machine is lost.

## Network rejection

- Missing, invalid, revoked, or destination-scoped keys receive `401`.
- Disallowed `Host` values and cross-origin browser requests receive `403`.
- Put bearer keys only in the `Authorization` header, never in a URL.
- Set `PJ_MCP_ENABLED=false` to remove the MCP route.
