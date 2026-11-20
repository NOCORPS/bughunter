# OpenClaw Integration

## MCP Integration (Recommended)

Configure OpenClaw with `bughunter.json` to expose all 8 `bughunter.*` tools over stdio, or connect over Streamable HTTP (`/mcp`) with Bearer authentication.

## CLI Fallback

```bash
bughunter analyze --repository . --json
bughunter investigate --repository . --issue "Issue summary" --json
```
