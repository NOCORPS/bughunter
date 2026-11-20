# BugHunter — Universal Debugging Integration Architecture

BugHunter is an open, extensible, provider-independent, and agent-independent debugging infrastructure written in Rust, with a companion Python SDK and standardized protocol servers.

Every external interface—CLI, Model Context Protocol (MCP), Agent2Agent (A2A), REST API v1, Rust SDK, Python SDK, and Agent Adapters—normalizes incoming requests into a single internal `DebuggingRequest` and executes through `BugHunterCore` under one unified `SecurityPolicy`.

## Architecture Overview

```mermaid
flowchart LR
    CLI["bughunter CLI"] --> Norm["Normalize -> DebuggingRequest"]
    MCP["MCP Server (Stdio / HTTP)"] --> Norm
    A2A["A2A Server"] --> Norm
    REST["REST API v1"] --> Norm
    SDKs["Rust & Python SDKs"] --> REST
    Agents["AgentAdapter Registry"] --> Norm

    Norm --> Sec["SecurityPolicy (Auth, RBAC, Rate Limit, Sandbox, Redaction, Audit)"]
    Sec --> Core["BugHunterCore Engine"]
    Core --> Res["Normalized DebuggingResult"]
```

### Core Principles

1. **Single Debugging Engine**: All integrations delegate to `BugHunterCore` ([engine.rs](crates/bughunter-core/src/engine.rs)). No debugging logic is duplicated in protocol layers.
2. **Provider Independence**: 15 built-in model provider adapters behind a unified `ModelProvider` trait with capability detection, retries, rate-limit handling, and graceful fallbacks.
3. **Agent Framework Independence**: Standardized MCP, A2A, and REST/HTTP interfaces plus `AgentAdapter` abstractions.
4. **Unified Security Controls**: Every integration enforces the same `SecurityPolicy` ([security.rs](crates/bughunter-core/src/security.rs))—authentication, RBAC permissions, rate limiting, sandbox path restrictions, secret redaction, and audit logging.
5. **Process-Isolated Plugins**: Third-party plugins run out-of-process via JSON-RPC over stdio (`SubprocessPluginRunner`), never inside the core process.

## Repository Layout

| Component | Path | Purpose |
| :--- | :--- | :--- |
| Core Engine & Security | [crates/bughunter-core](crates/bughunter-core/Cargo.toml) | `BugHunterCore`, `DebuggingRequest`/`DebuggingResult`, `SecurityPolicy`, `BugHunterError` |
| Model Providers | [crates/bughunter-providers](crates/bughunter-providers/Cargo.toml) | `ModelProvider` trait, OpenAI-compatible presets, Anthropic, Gemini, `ProviderRegistry` |
| Agent Adapters | [crates/bughunter-agents](crates/bughunter-agents/Cargo.toml) | `AgentAdapter` trait, `CoreBackedAgentAdapter`, `RemoteProtocolAgentAdapter`, `AgentRegistry` |
| MCP Server | [crates/bughunter-mcp](crates/bughunter-mcp/Cargo.toml) | MCP JSON-RPC 2.0 server over Stdio and Streamable HTTP (`POST /mcp`) |
| A2A Server | [crates/bughunter-a2a](crates/bughunter-a2a/Cargo.toml) | Agent Card (`/.well-known/agent.json`) and async A2A task endpoints |
| REST API v1 | [crates/bughunter-api](crates/bughunter-api/Cargo.toml) | Versioned `/api/v1/*` endpoints, OpenAPI 3.1 spec, and `unified_server_router` |
| Plugin System | [crates/bughunter-plugins](crates/bughunter-plugins/Cargo.toml) | `PluginManifest`, `PluginRegistry`, and out-of-process `SubprocessPluginRunner` |
| Rust SDK | [crates/bughunter-sdk](crates/bughunter-sdk/Cargo.toml) | Async `BugHunterClient` for the REST API v1 |
| CLI Binary | [crates/bughunter-cli](crates/bughunter-cli/Cargo.toml) | `bughunter` binary with human-readable and `--json` output |
| Python SDK | [sdk/python](sdk/python/pyproject.toml) | `BugHunterClient` & `AsyncBugHunterClient` (`bughunter_sdk`) |
| Agent Configs | [integrations](integrations/custom-mcp/README.md) | Ready-to-use MCP and CLI fallback guides for coding agents |

## Quick Start

### Prerequisites

- **Rust** (workspace tested on `rustc 1.99.1` / `cargo 1.99.1`)
- **Python 3.10+** (for `sdk/python` and plugin test harness)

### Build

```bash
cargo build --workspace
```

### Run CLI Debugging Workflows

```bash
# Run an end-to-end investigation with machine-readable JSON output
cargo run -p bughunter-cli -- investigate --repository . --issue "Fix auth token validation" --verify --json

# Analyze a repository
cargo run -p bughunter-cli -- analyze --repository . --json

# List all configured model providers and capabilities
cargo run -p bughunter-cli -- providers --json

# List all supported agent integrations
cargo run -p bughunter-cli -- integrations --json
```

### Start Servers

```bash
# 1. Start the unified HTTP server (REST API v1 + MCP Streamable HTTP + A2A)
BUGHUNTER_API_TOKEN=my-secret-token cargo run -p bughunter-cli -- serve --addr 127.0.0.1:8080

# 2. Start MCP over stdio (for local coding agents)
cargo run -p bughunter-cli -- mcp --transport stdio

# 3. Start standalone MCP Streamable HTTP
cargo run -p bughunter-cli -- mcp --transport http --addr 127.0.0.1:8081
```

## Documentation

- [Architecture & Security](docs/architecture.md) — Core workflows, normalization, RBAC, rate limiting, sandboxing, and secret redaction.
- [Model Providers](docs/providers.md) — `ModelProvider` trait, all 15 built-in adapters, environment variables, retries, and fallbacks.
- [REST API, MCP & A2A](docs/api-and-protocols.md) — Endpoints, OpenAPI 3.1, MCP tools, and A2A task lifecycle.
- [CLI & SDKs](docs/cli-and-sdks.md) — Full CLI reference, Rust SDK, and Python SDK usage.
- [Plugins & Agent Integrations](docs/plugins-and-agents.md) — Out-of-process plugin protocol, `AgentAdapter`, and coding agent guides.

## Verification & Testing

Run the full Rust workspace test suite (unit + mock-server integration tests) and the Python SDK tests:

```bash
cargo test --workspace
PYTHONPATH=sdk/python python3 -m unittest discover -s sdk/python/tests -v
```
