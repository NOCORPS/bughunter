use bughunter_core::{AuthContext, BugHunterError, Permission, SecurityPolicy};
use bughunter_plugins::{
    PluginCapabilityKind, PluginManifest, PluginRegistry, SubprocessPluginRunner,
};
use std::collections::HashSet;

#[tokio::test]
async fn test_out_of_process_plugin_execution_and_permissions() {
    let manifest = PluginManifest {
        name: "echo-analyzer".to_string(),
        version: "0.1.0".to_string(),
        compatibility: ">=0.1.0".to_string(),
        capabilities: vec![PluginCapabilityKind::RepositoryAnalyzer],
        required_permissions: vec![Permission::ReadRepository],
        config_schema: serde_json::json!({ "type": "object" }),
        entrypoint: "python3".to_string(),
        args: vec![
            "-c".to_string(),
            "import sys, json; req = json.loads(sys.stdin.readline()); print(json.dumps({'jsonrpc':'2.0','id':req['id'],'result':{'status':'ok','method':req['method']}}))".to_string(),
        ],
    };

    let mut registry = PluginRegistry::new();
    registry.register(manifest.clone()).unwrap();
    assert_eq!(registry.list().len(), 1);

    let runner = SubprocessPluginRunner::new(SecurityPolicy::default());
    let authorized = AuthContext::local_cli();
    let res = runner
        .invoke(&authorized, &manifest, "analyze", serde_json::json!({"repo": "."}))
        .await
        .unwrap();
    assert_eq!(res["status"], "ok");
    assert_eq!(res["method"], "analyze");

    // Unauthorized caller lacking ExecuteTools must be rejected
    let unpriv = AuthContext::scoped_agent(
        "restricted",
        HashSet::from([Permission::ReadRepository]),
        false,
    );
    let err = runner
        .invoke(&unpriv, &manifest, "analyze", serde_json::json!({}))
        .await
        .unwrap_err();
    assert!(matches!(err, BugHunterError::Authorization(_)));
}
