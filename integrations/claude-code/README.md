# Claude Code Integration

## MCP Integration (Recommended)

Register BugHunter in `.mcp.json` at the root of your repository or via the Claude Code CLI:

```bash
claude mcp add bughunter -- bughunter mcp --transport stdio
```

For remote Streamable HTTP deployments, start `bughunter serve --addr 127.0.0.1:8080 --token "$BUGHUNTER_API_TOKEN"` and point the MCP client to `http://127.0.0.1:8080/mcp` with a `Bearer` token.

## CLI Fallback

```bash
bughunter diagnose --repository . --issue "Auth token validation fails" --json
```
