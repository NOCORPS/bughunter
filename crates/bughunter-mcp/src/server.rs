use crate::protocol::{JsonRpcRequest, JsonRpcResponse, McpToolDefinition};
use bughunter_core::{
    AuthContext, BugHunterCore, BugHunterError, DebuggingRequest, DebuggingWorkflow,
    IntegrationSource,
};
use std::sync::Arc;

pub const MCP_PROTOCOL_VERSION: &str = "2025-03-26";

pub struct McpServer {
    core: Arc<BugHunterCore>,
}

impl McpServer {
    pub fn new(core: Arc<BugHunterCore>) -> Self {
        Self { core }
    }

    pub fn tool_definitions() -> Vec<McpToolDefinition> {
        let repo_issue_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "repository": { "type": "string", "description": "Local path or owner/repository identifier" },
                "issue": { "type": "string", "description": "Bug description or failing test details" },
                "files": { "type": "array", "items": { "type": "string" } },
                "verification_required": { "type": "boolean", "default": true }
            },
            "required": ["repository"]
        });

        let id_schema = serde_json::json!({
            "type": "object",
            "properties": {
                "investigation_id": { "type": "string" }
            },
            "required": ["investigation_id"]
        });

        vec![
            McpToolDefinition {
                name: "bughunter.investigate".to_string(),
                description: "Run a full end-to-end debugging investigation (analyze, reproduce, diagnose, patch, verify).".to_string(),
                input_schema: repo_issue_schema.clone(),
            },
            McpToolDefinition {
                name: "bughunter.analyze_repository".to_string(),
                description: "Analyze repository structure, languages, frameworks, and candidate files.".to_string(),
                input_schema: repo_issue_schema.clone(),
            },
            McpToolDefinition {
                name: "bughunter.reproduce".to_string(),
                description: "Reproduce a reported bug in an isolated sandbox.".to_string(),
                input_schema: repo_issue_schema.clone(),
            },
            McpToolDefinition {
                name: "bughunter.diagnose".to_string(),
                description: "Diagnose the root cause of an issue with evidence.".to_string(),
                input_schema: repo_issue_schema.clone(),
            },
            McpToolDefinition {
                name: "bughunter.generate_patch".to_string(),
                description: "Generate a candidate patch diff for a diagnosed issue.".to_string(),
                input_schema: repo_issue_schema.clone(),
            },
            McpToolDefinition {
                name: "bughunter.verify".to_string(),
                description: "Verify a candidate patch or fix against regression and security checks.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "repository": { "type": "string" },
                        "patch": { "type": "string" },
                        "issue": { "type": "string" }
                    },
                    "required": ["repository"]
                }),
            },
            McpToolDefinition {
                name: "bughunter.get_report".to_string(),
                description: "Retrieve the structured debugging report for an investigation.".to_string(),
                input_schema: id_schema.clone(),
            },
            McpToolDefinition {
                name: "bughunter.get_status".to_string(),
                description: "Query the current status of an investigation.".to_string(),
                input_schema: id_schema,
            },
        ]
    }

    pub async fn handle_request(
        &self,
        ctx: &AuthContext,
        source: IntegrationSource,
        req: JsonRpcRequest,
    ) -> Option<JsonRpcResponse> {
        let id = req.id.clone();

        // JSON-RPC notifications have no id and expect no response.
        if req.method.starts_with("notifications/") && id.is_none() {
            return None;
        }

        let response = match req.method.as_str() {
            "initialize" => JsonRpcResponse::ok(
                id,
                serde_json::json!({
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    },
                    "serverInfo": {
                        "name": "bughunter-mcp",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }),
            ),
            "ping" => JsonRpcResponse::ok(id, serde_json::json!({})),
            "tools/list" => JsonRpcResponse::ok(
                id,
                serde_json::json!({
                    "tools": Self::tool_definitions()
                }),
            ),
            "tools/call" => self.handle_tool_call(id, ctx, source, req.params).await,
            other => JsonRpcResponse::err(id, -32601, format!("method not found: {other}"), None),
        };

        Some(response)
    }

    async fn handle_tool_call(
        &self,
        id: Option<serde_json::Value>,
        ctx: &AuthContext,
        source: IntegrationSource,
        params: serde_json::Value,
    ) -> JsonRpcResponse {
        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::err(id, -32602, "missing tool 'name' parameter", None);
            }
        };
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let outcome: Result<serde_json::Value, BugHunterError> = match tool_name {
            "bughunter.investigate" => {
                self.run_workflow(ctx, source, DebuggingWorkflow::Investigate, args)
                    .await
            }
            "bughunter.analyze_repository" => {
                self.run_workflow(ctx, source, DebuggingWorkflow::AnalyzeRepository, args)
                    .await
            }
            "bughunter.reproduce" => {
                self.run_workflow(ctx, source, DebuggingWorkflow::Reproduce, args)
                    .await
            }
            "bughunter.diagnose" => {
                self.run_workflow(ctx, source, DebuggingWorkflow::Diagnose, args)
                    .await
            }
            "bughunter.generate_patch" => {
                self.run_workflow(ctx, source, DebuggingWorkflow::GeneratePatch, args)
                    .await
            }
            "bughunter.verify" => {
                self.run_workflow(ctx, source, DebuggingWorkflow::Verify, args)
                    .await
            }
            "bughunter.get_report" => {
                let inv_id = args
                    .get("investigation_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if inv_id.is_empty() {
                    Err(BugHunterError::Validation(
                        "investigation_id is required".to_string(),
                    ))
                } else {
                    self.core
                        .get_report(ctx, inv_id)
                        .and_then(|r| serde_json::to_value(r).map_err(|e| BugHunterError::Internal(e.to_string())))
                }
            }
            "bughunter.get_status" => {
                let inv_id = args
                    .get("investigation_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if inv_id.is_empty() {
                    Err(BugHunterError::Validation(
                        "investigation_id is required".to_string(),
                    ))
                } else {
                    self.core.get_investigation(ctx, inv_id).map(|rec| {
                        serde_json::json!({
                            "investigation_id": rec.id,
                            "status": rec.status,
                            "error": rec.error,
                            "updated_at": rec.updated_at,
                        })
                    })
                }
            }
            unknown => {
                return JsonRpcResponse::err(id, -32602, format!("unknown tool: {unknown}"), None);
            }
        };

        match outcome {
            Ok(structured) => {
                let text = serde_json::to_string_pretty(&structured).unwrap_or_default();
                JsonRpcResponse::ok(
                    id,
                    serde_json::json!({
                        "content": [{ "type": "text", "text": text }],
                        "structuredContent": structured,
                        "isError": false
                    }),
                )
            }
            Err(err) => JsonRpcResponse::ok(
                id,
                serde_json::json!({
                    "content": [{ "type": "text", "text": err.to_string() }],
                    "structuredContent": {
                        "error_code": err.code(),
                        "message": err.to_string()
                    },
                    "isError": true
                }),
            ),
        }
    }

    async fn run_workflow(
        &self,
        ctx: &AuthContext,
        source: IntegrationSource,
        workflow: DebuggingWorkflow,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, BugHunterError> {
        let repository = args
            .get("repository")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut req = DebuggingRequest::new(source, workflow, repository);
        req.issue = args.get("issue").and_then(|v| v.as_str()).map(String::from);
        req.patch = args.get("patch").and_then(|v| v.as_str()).map(String::from);
        req.verification_required = args
            .get("verification_required")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if let Some(files) = args.get("files").and_then(|v| v.as_array()) {
            req.files = files
                .iter()
                .filter_map(|f| f.as_str().map(String::from))
                .collect();
        }
        let result = self.core.execute(ctx, req).await?;
        serde_json::to_value(result).map_err(|e| BugHunterError::Internal(e.to_string()))
    }
}
