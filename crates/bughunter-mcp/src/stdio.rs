use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::server::McpServer;
use bughunter_core::{AuthContext, IntegrationSource};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Runs the MCP server over stdio (newline-delimited JSON-RPC 2.0).
pub async fn serve_stdio(server: McpServer, ctx: AuthContext) -> std::io::Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut lines = BufReader::new(stdin).lines();

    while let Some(line) = lines.next_line().await? {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let resp_opt = match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(req) => {
                server
                    .handle_request(&ctx, IntegrationSource::McpStdio, req)
                    .await
            }
            Err(e) => Some(JsonRpcResponse::err(
                None,
                -32700,
                format!("parse error: {e}"),
                None,
            )),
        };

        if let Some(resp) = resp_opt {
            let serialized = serde_json::to_string(&resp).unwrap_or_default();
            stdout.write_all(serialized.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}
