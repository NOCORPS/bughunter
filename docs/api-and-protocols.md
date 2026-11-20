# REST API, MCP Server, and A2A Integration

Running `bughunter serve --addr 127.0.0.1:8080` mounts the unified router ([router.rs](../crates/bughunter-api/src/router.rs)) combining REST API v1, MCP Streamable HTTP, and A2A on a single server. When `--token` / `BUGHUNTER_API_TOKEN` is set, Bearer authentication is enforced on all protected endpoints.

## 1. Versioned REST API (`/api/v1`)

| Method | Path | Description |
| :--- | :--- | :--- |
| `POST` | `/api/v1/investigations` | Start an asynchronous debugging investigation (`202 Accepted`) |
| `GET` | `/api/v1/investigations/{id}` | Query investigation status and result |
| `POST` | `/api/v1/investigations/{id}/cancel` | Cancel an active investigation |
| `GET` | `/api/v1/investigations/{id}/report` | Retrieve the structured `Report` |
| `POST` | `/api/v1/repositories/analyze` | Synchronous repository analysis |
| `POST` | `/api/v1/bugs/reproduce` | Synchronous bug reproduction |
| `POST` | `/api/v1/patches/generate` | Synchronous patch generation |
| `POST` | `/api/v1/patches/verify` | Synchronous patch verification |
| `GET` | `/api/v1/providers` | List configured model providers and capabilities |
| `GET` | `/api/v1/health` | Service health and investigation counts |
| `GET` | `/api/v1/openapi.json` | OpenAPI `3.1.0` specification |

### Example: Start Investigation

```bash
curl -X POST http://127.0.0.1:8080/api/v1/investigations \
  -H "Authorization: Bearer $BUGHUNTER_API_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "repository": "owner/repository",
    "issue": "Fix authentication failure",
    "verification_required": true
  }'
```

## 2. Model Context Protocol (MCP) Server

Implemented in `bughunter-mcp` ([server.rs](../crates/bughunter-mcp/src/server.rs)), supporting protocol version `2025-03-26`:

- **Stdio transport**: `bughunter mcp --transport stdio` (newline-delimited JSON-RPC 2.0 over stdin/stdout)
- **Streamable HTTP transport**: `POST /mcp` (returns JSON or `text/event-stream` SSE when `Accept: text/event-stream` is sent)

### Exposed MCP Tools

1. `bughunter.investigate`
2. `bughunter.analyze_repository`
3. `bughunter.reproduce`
4. `bughunter.diagnose`
5. `bughunter.generate_patch`
6. `bughunter.verify`
7. `bughunter.get_report`
8. `bughunter.get_status`

All MCP tool calls execute via `BugHunterCore` and return both `content` (text) and `structuredContent` (JSON), respecting `SecurityPolicy` permissions.

## 3. Agent2Agent (A2A) Interface

Implemented in `bughunter-a2a` ([router.rs](../crates/bughunter-a2a/src/router.rs)):

| Method | Path | Description |
| :--- | :--- | :--- |
| `GET` | `/.well-known/agent.json` | Publishes the BugHunter `AgentCard` (capabilities, auth, skills) |
| `POST` | `/a2a/tasks/send` | Submits an async debugging task (`202 Accepted`) |
| `GET` | `/a2a/tasks/{id}` | Retrieves task status and `DebuggingResult` |
| `POST` | `/a2a/tasks/{id}/cancel` | Cancels an active task |

### Example A2A Task Payload

```json
{
  "capability": "debugging",
  "repository": "owner/repository",
  "issue": "Fix authentication failure",
  "verification_required": true
}
```

Supported `capability` values: `debugging`/`investigate`, `analyze`/`repository_analysis`, `reproduce`, `diagnose`, `patch`/`generate_patch`, `verify`/`verification`.
