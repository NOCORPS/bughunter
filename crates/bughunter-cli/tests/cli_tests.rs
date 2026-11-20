use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn test_cli_json_outputs_and_mcp_stdio() {
    let bin = env!("CARGO_BIN_EXE_bughunter");

    // 1. bughunter providers --json
    let out = Command::new(bin)
        .args(["providers", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let providers: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(providers.as_array().unwrap().len() >= 15);

    // 2. bughunter integrations --json
    let out = Command::new(bin)
        .args(["integrations", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let integrations: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(integrations.as_array().unwrap().len(), 12);

    // 3. bughunter investigate --repository . --issue "test" --verify --json
    let out = Command::new(bin)
        .args([
            "investigate",
            "--repository",
            ".",
            "--issue",
            "CLI integration test",
            "--verify",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let inv: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(inv["status"], "completed");
    assert_eq!(inv["verification"]["verified"], true);

    // 4. bughunter mcp --transport stdio
    let mut child = Command::new(bin)
        .args(["mcp", "--transport", "stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let stdin = child.stdin.as_mut().unwrap();
        writeln!(
            stdin,
            "{}",
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})
        )
        .unwrap();
        writeln!(
            stdin,
            "{}",
            serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})
        )
        .unwrap();
    }

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let stdout_str = String::from_utf8(output.stdout).unwrap();
    let lines: Vec<&str> = stdout_str.lines().collect();
    assert_eq!(lines.len(), 2);
    let init_resp: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "bughunter-mcp");
}
