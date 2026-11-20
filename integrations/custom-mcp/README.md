# Custom MCP-Compatible Agents

BugHunter supports both standard MCP transports:

1. **Stdio** (`mcp-stdio.json`): `bughunter mcp --transport stdio`
2. **Streamable HTTP** (`mcp-http.json`): `bughunter serve --addr 127.0.0.1:8080 --token "$BUGHUNTER_API_TOKEN"` (endpoint `POST /mcp`)

## CLI Fallback

Every debugging workflow is available with `--json` for machine-readable output:

```bash
bughunter investigate --repository . --issue "..." --verify --json
```
