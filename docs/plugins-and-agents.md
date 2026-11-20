# Plugin Architecture & Agent Integrations

## Out-of-Process Plugin System (`bughunter-plugins`)

Third-party plugins extend BugHunter without ever executing untrusted code inside the core process ([lib.rs](../crates/bughunter-plugins/src/lib.rs)).

### Supported Plugin Capabilities (`PluginCapabilityKind`)

- `ModelProvider`
- `RepositoryAnalyzer`
- `DebuggingTool`
- `TestRunner`
- `FrameworkDetector`
- `VerificationStrategy`
- `ReportExporter`
- `AgentAdapter`

### Plugin Manifest (`PluginManifest`)

Every plugin declares:
- `name`, `version`, `compatibility`
- `capabilities` (at least one `PluginCapabilityKind`)
- `required_permissions` (list of `bughunter_core::Permission`)
- `config_schema` (JSON Schema)
- `entrypoint` and `args`

### Isolation & Security Enforcement

`SubprocessPluginRunner::invoke`:
1. Validates the `PluginManifest`.
2. Verifies the caller holds `Permission::ExecuteTools` and every permission listed in `manifest.required_permissions`.
3. Spawns the plugin executable with `.env_clear()`, piped stdin/stdout, and null stderr.
4. Sends a single JSON-RPC 2.0 request line and awaits a single JSON-RPC 2.0 response line within `SecurityPolicy.limits.max_execution_seconds`.
5. Enforces `SecurityPolicy.limits.max_output_bytes` on the response.

## Agent Integration System (`bughunter-agents`)

Defined in [types.rs](../crates/bughunter-agents/src/types.rs) and [adapters.rs](../crates/bughunter-agents/src/adapters.rs):

```rust
#[async_trait]
pub trait AgentAdapter: Send + Sync {
    async fn discover(&self) -> Result<AgentCapabilities>;
    async fn submit_task(&self, task: AgentTask) -> Result<AgentTaskResult>;
    async fn get_status(&self, task_id: &str) -> Result<AgentTaskStatus>;
}
```

### Supported Agent Frameworks & Preferred Protocols

| Framework | ID | Preferred Protocol |
| :--- | :--- | :--- |
| OpenClaw | `openclaw` | MCP |
| Hermes Agent | `hermes_agent` | MCP |
| OpenCode | `opencode` | MCP |
| Claude Code | `claude_code` | MCP |
| OpenHands | `openhands` | MCP |
| Google ADK | `google_adk` | A2A |
| LangGraph | `langgraph` | A2A |
| CrewAI | `crewai` | A2A |
| AutoGen | `autogen` | A2A |
| SWE-agent | `swe_agent` | HTTP JSON |
| OpenAI Agents SDK | `openai_agents_sdk` | HTTP JSON |
| Custom | `<name>` | HTTP JSON |

### Coding Agent Configurations

Ready-to-use MCP configs and CLI fallback instructions are located in `integrations/`:
- [OpenCode](../integrations/opencode/README.md) ([opencode.json](../integrations/opencode/opencode.json))
- [Claude Code](../integrations/claude-code/README.md) ([mcp.json](../integrations/claude-code/mcp.json))
- [OpenClaw](../integrations/openclaw/README.md) ([bughunter.json](../integrations/openclaw/bughunter.json))
- [Hermes Agent](../integrations/hermes/README.md) ([mcp-config.json](../integrations/hermes/mcp-config.json))
- [OpenHands](../integrations/openhands/README.md) ([config.toml](../integrations/openhands/config.toml))
- [Custom MCP Agents](../integrations/custom-mcp/README.md) ([mcp-stdio.json](../integrations/custom-mcp/mcp-stdio.json), [mcp-http.json](../integrations/custom-mcp/mcp-http.json))
