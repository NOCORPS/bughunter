# Architecture & Security

## Request & Result Normalization

BugHunter enforces a strict separation between **interface layers** (CLI, MCP, A2A, REST, SDKs, Agent Adapters) and the **debugging engine** (`BugHunterCore`).

1. Every incoming call is converted into a [DebuggingRequest](../crates/bughunter-core/src/types.rs) containing:
   - `request_id` (UUID v4)
   - `source`: `IntegrationSource` (`Cli`, `McpStdio`, `McpHttp`, `A2a`, `RestApi`, `RustSdk`, `PythonSdk`, `AgentAdapter(String)`, `Internal`)
   - `workflow`: `DebuggingWorkflow` (`Investigate`, `AnalyzeRepository`, `Reproduce`, `Diagnose`, `GeneratePatch`, `Verify`)
   - `repository`, `issue`, `target_ref`, `files`, `patch`, `verification_required`, `provider_hint`, `model_hint`, `parameters`, `metadata`
2. `BugHunterCore::execute` (synchronous) or `BugHunterCore::start_investigation` (asynchronous) authorizes the request via `SecurityPolicy` and runs the core pipeline.
3. All outputs are returned as a normalized [DebuggingResult](../crates/bughunter-core/src/types.rs) with `InvestigationStatus`, `RepositoryAnalysis`, optional `ReproductionResult`, `DiagnosisResult`, `PatchResult`, `VerificationResult`, `Report`, and `ExecutionMetrics`.

### Investigation Lifecycle

```
Pending -> Running -> Completed
                   -> Failed
                   -> Cancelled (via cancel_investigation)
                   -> AwaitingApproval (when human approval is required)
```

## Unified Security Policy

All integrations share [SecurityPolicy](../crates/bughunter-core/src/security.rs):

### Authentication & Scoped Credentials

- `SecurityPolicy::register_token(token, AuthContext)` registers bearer tokens mapped to an `AuthContext` (`principal`, `auth_method`, `scopes`, `credential_id`, `human_approved`).
- `AuthContext::local_cli()` grants full local permissions; `AuthContext::scoped_agent(...)` restricts remote agents to explicit `Permission` scopes.
- HTTP transports (`bughunter-api`, `bughunter-mcp` HTTP, `bughunter-a2a`) extract `Authorization: Bearer <token>` and call `authenticate_bearer`.

### RBAC Permissions

| Workflow / Action | Required `Permission` |
| :--- | :--- |
| `Investigate` | `Permission::Investigate` |
| `AnalyzeRepository` | `Permission::ReadRepository` |
| `Reproduce` | `Permission::Reproduce` |
| `Diagnose` | `Permission::Diagnose` |
| `GeneratePatch` | `Permission::GeneratePatch` |
| `Verify` | `Permission::VerifyPatch` |
| Plugin / Tool Execution | `Permission::ExecuteTools` + plugin's `required_permissions` |
| Full Access | `Permission::Admin` |

### Rate Limiting, Resource Limits & Sandbox Controls

- **Rate Limiter**: Sliding 60-second window per `principal` (`ResourceLimits::rate_limit_per_minute`, default `60`).
- **Resource Limits** (`ResourceLimits::default()`):
  - `max_execution_seconds`: `120`
  - `max_files_inspected`: `500`
  - `max_output_bytes`: `1_048_576` (1 MiB)
  - `max_concurrent_investigations`: `16`
- **Sandbox Policy** (`SandboxPolicy::default()`):
  - Rejects empty repository targets and path traversal (`..`).
  - Blocks sensitive absolute path prefixes (`/etc`, `/root`, `/proc`, `/sys`).
  - `allow_network_for_repro`: `false`
  - `allow_filesystem_write`: `false`
  - `require_human_approval_for_patch`: configurable gate returning `BugHunterError::ApprovalRequired` when `!ctx.human_approved`.

### Secret Redaction & Audit Logging

- `SecretRedactor::redact` scrubs `Bearer ...`, `sk-...`, `ghp_...`, and `api_key`/`secret`/`password`/`token` key-value pairs before storing or returning outputs and audit records.
- `AuditLogger` (`InMemoryAuditLogger` by default) records every authorization decision with timestamp, principal, `IntegrationSource`, action, redacted target, and outcome.

## Normalized Errors

[BugHunterError](../crates/bughunter-core/src/error.rs) maps cleanly across CLI, HTTP, MCP, and SDKs:

| Variant | `code()` | HTTP Status | Retryable |
| :--- | :--- | :--- | :--- |
| `Authentication` | `AUTHENTICATION_ERROR` | `401` | No |
| `Authorization` | `AUTHORIZATION_ERROR` | `403` | No |
| `Validation` | `VALIDATION_ERROR` | `400` | No |
| `NotFound` | `NOT_FOUND` | `404` | No |
| `RateLimited` | `RATE_LIMITED` | `429` | Yes |
| `Timeout` | `TIMEOUT` | `504` | Yes |
| `Provider` | `PROVIDER_ERROR` | `status_code` or `502` | `retryable` |
| `SandboxViolation` | `SANDBOX_VIOLATION` | `403` | No |
| `ApprovalRequired` | `APPROVAL_REQUIRED` | `409` | No |
| `Cancelled` | `CANCELLED` | `409` | No |
| `Unsupported` | `UNSUPPORTED_CAPABILITY` | `501` | No |
| `Internal` | `INTERNAL_ERROR` | `500` | No |
