# Hermes Agent Integration

## MCP Integration (Recommended)

Use `mcp-config.json` to register BugHunter as a standard MCP stdio server for Hermes.

## A2A / REST Integration

Hermes can also delegate asynchronous debugging tasks to BugHunter's A2A endpoint (`/.well-known/agent.json`, `/a2a/tasks/send`) when `bughunter serve` is running.

## CLI Fallback

```bash
bughunter investigate --repository . --issue "Describe failure" --json
```
