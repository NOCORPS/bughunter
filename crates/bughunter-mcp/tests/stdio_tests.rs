use bughunter_core::{AuthContext, BugHunterCore, IntegrationSource, Permission};
use bughunter_mcp::{JsonRpcRequest, McpServer};
use std::collections::HashSet;
use std::sync::Arc;

#[tokio::test]
async fn test_mcp_tools_list_and_security_enforcement() {
    let core = Arc::new(BugHunterCore::default());
    let server = McpServer::new(core);
    let admin = AuthContext::local_cli();

    let list_resp = server
        .handle_request(
            &admin,
            IntegrationSource::McpStdio,
            JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: Some(serde_json::json!(1)),
                method: "tools/list".into(),
                params: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let tools = list_resp.result.unwrap()["tools"].as_array().unwrap().clone();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    for expected in [
        "bughunter.investigate",
        "bughunter.analyze_repository",
        "bughunter.reproduce",
        "bughunter.diagnose",
        "bughunter.generate_patch",
        "bughunter.verify",
        "bughunter.get_report",
        "bughunter.get_status",
    ] {
        assert!(names.contains(&expected), "missing MCP tool {expected}");
    }

    // Security control: read-only agent cannot call bughunter.generate_patch
    let read_only = AuthContext::scoped_agent(
        "readonly-agent",
        HashSet::from([Permission::ReadRepository]),
        false,
    );
    let denied = server
        .handle_request(
            &read_only,
            IntegrationSource::McpStdio,
            JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: Some(serde_json::json!(2)),
                method: "tools/call".into(),
                params: serde_json::json!({
                    "name": "bughunter.generate_patch",
                    "arguments": { "repository": ".", "issue": "test" }
                }),
            },
        )
        .await
        .unwrap();
    assert_eq!(denied.result.unwrap()["isError"], true);
}
