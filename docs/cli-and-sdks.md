# CLI & SDK Reference

## BugHunter CLI (`bughunter`)

All commands support the global `--json` flag for machine-readable output ([main.rs](../crates/bughunter-cli/src/main.rs)).

| Command | Description | Key Flags |
| :--- | :--- | :--- |
| `bughunter investigate` | Full end-to-end investigation | `-r/--repository`, `-i/--issue`, `-f/--files`, `--verify`, `--provider`, `--model` |
| `bughunter analyze` | Analyze repository structure & languages | `-r/--repository`, `-f/--files` |
| `bughunter reproduce` | Reproduce a reported bug | `-r/--repository`, `-i/--issue` |
| `bughunter diagnose` | Diagnose root cause with evidence | `-r/--repository`, `-i/--issue` |
| `bughunter fix` | Generate a candidate patch diff | `-r/--repository`, `-i/--issue` |
| `bughunter verify` | Verify a patch or fix | `-r/--repository`, `-p/--patch`, `-i/--issue` |
| `bughunter report` | Fetch report by `--id` or analyze `--repository` | `--id`, `-r/--repository` |
| `bughunter mcp` | Run MCP server (`stdio` or `http`) | `--transport`, `--addr` |
| `bughunter serve` | Run unified REST + MCP + A2A HTTP server | `--addr`, `--token` (or `BUGHUNTER_API_TOKEN`) |
| `bughunter providers` | List all 15 model providers & capabilities | `--json` |
| `bughunter integrations` | List all 12 agent adapters & protocols | `--json` |

## Rust SDK (`bughunter-sdk`)

Implemented in [lib.rs](../crates/bughunter-sdk/src/lib.rs):

```rust
use bughunter_sdk::{BugHunterClient, StartInvestigationRequest};
use std::time::Duration;

#[tokio::main]
async fn main() -> bughunter_core::Result<()> {
    let client = BugHunterClient::new("http://127.0.0.1:8080", Some("my-token".into()));
    let record = client
        .start_investigation(
            &StartInvestigationRequest::new("owner/repository")
                .with_issue("Fix authentication failure")
                .with_verification(true),
        )
        .await?;

    let result = client
        .wait_for_investigation(&record.id, Duration::from_millis(250), Duration::from_secs(60))
        .await?;
    println!("Status: {:?}, Summary: {}", result.status, result.summary);

    let report = client.get_report(&record.id).await?;
    println!("Report: {}", report.title);
    Ok(())
}
```

## Python SDK (`bughunter_sdk`)

Implemented in [sdk/python/bughunter_sdk](../sdk/python/bughunter_sdk/client.py) using only the Python standard library (zero runtime dependencies):

```python
from bughunter_sdk import BugHunterClient, AuthenticationError, NotFoundError

client = BugHunterClient("http://127.0.0.1:8080", api_key="my-token")

record = client.start_investigation(
    repository="owner/repository",
    issue="Fix authentication failure",
    verification_required=True,
)

completed = client.wait_for_investigation(record.id, poll_interval=0.25, timeout=60.0)
report = client.get_report(record.id)
print(report.title, report.executive_summary)
```

An `AsyncBugHunterClient` is also provided with identical methods (`await client.start_investigation(...)`, `await client.wait_for_investigation(...)`, `await client.get_report(...)`, `await client.cancel_investigation(...)`).

### Python Error Hierarchy

- `BugHunterError` (base)
  - `AuthenticationError` (HTTP `401`)
  - `AuthorizationError` (HTTP `403`)
  - `ValidationError` (HTTP `400` / `422`)
  - `NotFoundError` (HTTP `404`)
  - `RateLimitError` (HTTP `429`)
  - `TimeoutError`
