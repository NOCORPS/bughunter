use bughunter_agents::AgentRegistry;
use bughunter_api::unified_server_router;
use bughunter_core::{AuthContext, BugHunterCore, Permission};
use bughunter_providers::ProviderRegistry;
use bughunter_sdk::{BugHunterClient, StartInvestigationRequest};
use std::sync::Arc;
use std::time::Duration;

async fn start_test_server(require_auth: bool) -> (String, Arc<BugHunterCore>) {
    let core = Arc::new(BugHunterCore::default());
    core.security.register_token(
        "test-token",
        AuthContext::scoped_agent("ci-agent", Permission::standard_agent(), true),
    );
    let providers = Arc::new(ProviderRegistry::with_builtin_presets());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let app = unified_server_router(Arc::clone(&core), providers, base_url.clone(), require_auth);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (base_url, core)
}

#[tokio::test]
async fn test_rest_api_sdk_a2a_and_mcp_http_end_to_end() {
    let (base_url, core) = start_test_server(true).await;

    // 1. Rust SDK: health + auth failure + full investigation lifecycle
    let unauth_sdk = BugHunterClient::new(&base_url, None);
    assert_eq!(unauth_sdk.health().await.unwrap().status, "ok");
    assert!(unauth_sdk
        .start_investigation(&StartInvestigationRequest::new("owner/repo"))
        .await
        .is_err());

    let sdk = BugHunterClient::new(&base_url, Some("test-token".into()));
    let inv = sdk
        .start_investigation(
            &StartInvestigationRequest::new("owner/repository")
                .with_issue("Fix auth with sk-secret12345678901234")
                .with_verification(true),
        )
        .await
        .unwrap();

    let completed = sdk
        .wait_for_investigation(&inv.id, Duration::from_millis(20), Duration::from_secs(5))
        .await
        .unwrap();
    assert!(completed.verification.unwrap().verified);
    // Secret redaction check
    assert!(!completed.summary.contains("sk-secret12345678901234"));

    let report = sdk.get_report(&inv.id).await.unwrap();
    assert_eq!(report.investigation_id, inv.id);

    // 2. REST API OpenAPI + Providers + Sync endpoints
    let http = reqwest::Client::new();
    let openapi: serde_json::Value = http
        .get(format!("{base_url}/api/v1/openapi.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(openapi["openapi"], "3.1.0");

    let providers_resp: serde_json::Value = http
        .get(format!("{base_url}/api/v1/providers"))
        .bearer_auth("test-token")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(providers_resp["providers"].as_array().unwrap().len() >= 15);

    // 3. MCP Streamable HTTP
    let mcp_init: serde_json::Value = http
        .post(format!("{base_url}/mcp"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(mcp_init["result"]["serverInfo"]["name"], "bughunter-mcp");

    let mcp_call: serde_json::Value = http
        .post(format!("{base_url}/mcp"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "bughunter.analyze_repository",
                "arguments": { "repository": "." }
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(mcp_call["result"]["isError"], false);

    // 4. A2A Agent Card + Task Submission + Cancellation
    let card: serde_json::Value = http
        .get(format!("{base_url}/.well-known/agent.json"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(card["name"], "BugHunter");

    let a2a_resp = http
        .post(format!("{base_url}/a2a/tasks/send"))
        .bearer_auth("test-token")
        .json(&serde_json::json!({
            "capability": "debugging",
            "repository": "owner/repository",
            "issue": "Fix authentication failure",
            "verification_required": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(a2a_resp.status(), reqwest::StatusCode::ACCEPTED);
    let a2a_json: serde_json::Value = a2a_resp.json().await.unwrap();
    let task_id = a2a_json["task_id"].as_str().unwrap();

    let cancel_json: serde_json::Value = http
        .post(format!("{base_url}/a2a/tasks/{task_id}/cancel"))
        .bearer_auth("test-token")
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cancel_json["task_id"], task_id);

    // 5. Agent Adapter Registry
    let agents = AgentRegistry::with_builtin_adapters(core, AuthContext::local_cli());
    let caps = agents.list_capabilities().await;
    assert_eq!(caps.len(), 12);
}
