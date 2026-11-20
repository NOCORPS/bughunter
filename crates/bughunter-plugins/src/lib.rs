use bughunter_core::{
    AuthContext, BugHunterError, IntegrationSource, Permission, Result, SecurityPolicy,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapabilityKind {
    ModelProvider,
    RepositoryAnalyzer,
    DebuggingTool,
    TestRunner,
    FrameworkDetector,
    VerificationStrategy,
    ReportExporter,
    AgentAdapter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub compatibility: String,
    pub capabilities: Vec<PluginCapabilityKind>,
    pub required_permissions: Vec<Permission>,
    pub config_schema: serde_json::Value,
    pub entrypoint: String,
    #[serde(default)]
    pub args: Vec<String>,
}

impl PluginManifest {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(BugHunterError::Validation(
                "plugin manifest name must not be empty".to_string(),
            ));
        }
        if self.version.trim().is_empty() {
            return Err(BugHunterError::Validation(
                "plugin manifest version must not be empty".to_string(),
            ));
        }
        if self.entrypoint.trim().is_empty() {
            return Err(BugHunterError::Validation(
                "plugin manifest entrypoint must not be empty".to_string(),
            ));
        }
        if self.capabilities.is_empty() {
            return Err(BugHunterError::Validation(
                "plugin manifest must declare at least one capability".to_string(),
            ));
        }
        Ok(())
    }
}

/// Executes plugins out-of-process via JSON-RPC over stdio.
/// Untrusted plugins are NEVER executed inside the BugHunter Core process.
pub struct SubprocessPluginRunner {
    security: SecurityPolicy,
}

impl SubprocessPluginRunner {
    pub fn new(security: SecurityPolicy) -> Self {
        Self { security }
    }

    pub async fn invoke(
        &self,
        ctx: &AuthContext,
        manifest: &PluginManifest,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        manifest.validate()?;

        // Enforce that caller holds ManagePlugins or ExecuteTools and every permission required by the plugin.
        self.security.authorize_permission(
            ctx,
            IntegrationSource::Internal,
            Permission::ExecuteTools,
            &manifest.name,
        )?;
        for perm in &manifest.required_permissions {
            self.security.authorize_permission(
                ctx,
                IntegrationSource::Internal,
                *perm,
                &manifest.name,
            )?;
        }

        let mut child = Command::new(&manifest.entrypoint)
            .args(&manifest.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .env_clear()
            .spawn()
            .map_err(|e| {
                BugHunterError::Internal(format!(
                    "failed to spawn plugin '{}' ({}): {e}",
                    manifest.name, manifest.entrypoint
                ))
            })?;

        let req_json = serde_json::json!({
            "jsonrpc": "2.0",
            "id": uuid::Uuid::new_v4().to_string(),
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_string(&req_json)
            .map_err(|e| BugHunterError::Internal(e.to_string()))?;
        line.push('\n');

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(line.as_bytes())
                .await
                .map_err(|e| BugHunterError::Internal(e.to_string()))?;
            stdin
                .flush()
                .await
                .map_err(|e| BugHunterError::Internal(e.to_string()))?;
        }

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| BugHunterError::Internal("plugin stdout unavailable".into()))?;
        let mut reader = BufReader::new(stdout).lines();

        let timeout = Duration::from_secs(self.security.limits.max_execution_seconds);
        let resp_line = tokio::time::timeout(timeout, reader.next_line())
            .await
            .map_err(|_| {
                let _ = child.start_kill();
                BugHunterError::Timeout(format!(
                    "plugin '{}' timed out after {}s",
                    manifest.name, self.security.limits.max_execution_seconds
                ))
            })?
            .map_err(|e| BugHunterError::Internal(e.to_string()))?
            .ok_or_else(|| {
                BugHunterError::Internal(format!("plugin '{}' closed stdout without response", manifest.name))
            })?;

        let _ = child.wait().await;

        if resp_line.len() > self.security.limits.max_output_bytes {
            return Err(BugHunterError::SandboxViolation(format!(
                "plugin '{}' output exceeded {} bytes",
                manifest.name, self.security.limits.max_output_bytes
            )));
        }

        let resp_val: serde_json::Value = serde_json::from_str(&resp_line)
            .map_err(|e| BugHunterError::Internal(format!("invalid plugin JSON-RPC response: {e}")))?;
        if let Some(err) = resp_val.get("error") {
            return Err(BugHunterError::Internal(format!(
                "plugin '{}' returned error: {err}",
                manifest.name
            )));
        }
        Ok(resp_val.get("result").cloned().unwrap_or(serde_json::Value::Null))
    }
}

#[derive(Default)]
pub struct PluginRegistry {
    plugins: BTreeMap<String, PluginManifest>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, manifest: PluginManifest) -> Result<()> {
        manifest.validate()?;
        self.plugins.insert(manifest.name.clone(), manifest);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Result<&PluginManifest> {
        self.plugins
            .get(name)
            .ok_or_else(|| BugHunterError::NotFound(format!("plugin '{name}' not found")))
    }

    pub fn list(&self) -> Vec<&PluginManifest> {
        self.plugins.values().collect()
    }
}
