# OpenHands Integration

## MCP Integration (Recommended)

Add the `[[mcp.stdio_servers]]` block from `config.toml` into your OpenHands configuration.

## CLI Fallback

```bash
bughunter reproduce --repository . --issue "Failing test description" --json
bughunter verify --repository . --patch "..." --json
```
