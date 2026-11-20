use crate::types::{
    AgentAdapter, AgentCapabilities, AgentFrameworkKind, AgentProtocol, AgentTask, AgentTaskResult,
    AgentTaskState, AgentTaskStatus,
};
use async_trait::async_trait;
use bughunter_core::{AuthContext, BugHunterCore, BugHunterError, InvestigationStatus, Result};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Adapter that routes agent tasks through BugHunterCore using standardized normalization.
pub struct CoreBackedAgentAdapter {
    pub name: String,
    pub framework: AgentFrameworkKind,
    pub protocol: AgentProtocol,
    core: Arc<BugHunterCore>,
    auth: AuthContext,
}

impl CoreBackedAgentAdapter {
    pub fn new(
        name: impl Into<String>,
        framework: AgentFrameworkKind,
        core: Arc<BugHunterCore>,
        auth: AuthContext,
    ) -> Self {
        let protocol = framework.preferred_protocol();
        Self {
            name: name.into(),
            framework,
            protocol,
            core,
            auth,
        }
    }
}

#[async_trait]
impl AgentAdapter for CoreBackedAgentAdapter {
    async fn discover(&self) -> Result<AgentCapabilities> {
        Ok(AgentCapabilities {
            name: self.name.clone(),
            framework: self.framework.clone(),
            protocol: self.protocol.clone(),
            supports_async_tasks: true,
            supports_cancellation: true,
            supported_workflows: vec![
                "investigate".to_string(),
                "analyze_repository".to_string(),
                "reproduce".to_string(),
                "diagnose".to_string(),
                "generate_patch".to_string(),
                "verify".to_string(),
            ],
            endpoint: None,
        })
    }

    async fn submit_task(&self, task: AgentTask) -> Result<AgentTaskResult> {
        let mut req = task.request;
        req.source = bughunter_core::IntegrationSource::AgentAdapter(self.framework.id());
        let record = self.core.start_investigation(&self.auth, req).await?;
        Ok(AgentTaskResult {
            task_id: record.id,
            state: AgentTaskState::Submitted,
            debugging_result: record.result,
            raw_payload: None,
        })
    }

    async fn get_status(&self, task_id: &str) -> Result<AgentTaskStatus> {
        let rec = self.core.get_investigation(&self.auth, task_id)?;
        let state = match rec.status {
            InvestigationStatus::Pending => AgentTaskState::Submitted,
            InvestigationStatus::Running | InvestigationStatus::AwaitingApproval => {
                AgentTaskState::Working
            }
            InvestigationStatus::Completed => AgentTaskState::Completed,
            InvestigationStatus::Failed => AgentTaskState::Failed,
            InvestigationStatus::Cancelled => AgentTaskState::Cancelled,
        };
        Ok(AgentTaskStatus {
            task_id: rec.id,
            state,
            message: rec.error,
        })
    }
}

/// Standardized HTTP/A2A-compatible remote agent adapter.
pub struct RemoteProtocolAgentAdapter {
    pub name: String,
    pub framework: AgentFrameworkKind,
    pub protocol: AgentProtocol,
    pub endpoint: String,
    pub bearer_token: Option<String>,
    client: reqwest::Client,
}

impl RemoteProtocolAgentAdapter {
    pub fn new(
        name: impl Into<String>,
        framework: AgentFrameworkKind,
        protocol: AgentProtocol,
        endpoint: impl Into<String>,
        bearer_token: Option<String>,
    ) -> Self {
        Self {
            name: name.into(),
            framework,
            protocol,
            endpoint: endpoint.into(),
            bearer_token,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl AgentAdapter for RemoteProtocolAgentAdapter {
    async fn discover(&self) -> Result<AgentCapabilities> {
        Ok(AgentCapabilities {
            name: self.name.clone(),
            framework: self.framework.clone(),
            protocol: self.protocol.clone(),
            supports_async_tasks: true,
            supports_cancellation: true,
            supported_workflows: vec!["debugging".to_string()],
            endpoint: Some(self.endpoint.clone()),
        })
    }

    async fn submit_task(&self, task: AgentTask) -> Result<AgentTaskResult> {
        let url = format!("{}/tasks/send", self.endpoint.trim_end_matches('/'));
        let mut req = self.client.post(&url).json(&task);
        if let Some(tok) = &self.bearer_token {
            req = req.bearer_auth(tok);
        }
        let resp = req.send().await.map_err(|e| BugHunterError::Provider {
            provider: self.name.clone(),
            message: e.to_string(),
            retryable: true,
            status_code: None,
        })?;
        let status = resp.status();
        let val: serde_json::Value = resp.json().await.unwrap_or_default();
        if !status.is_success() {
            return Err(BugHunterError::Provider {
                provider: self.name.clone(),
                message: format!("HTTP {status}: {val}"),
                retryable: status.is_server_error(),
                status_code: Some(status.as_u16()),
            });
        }
        Ok(AgentTaskResult {
            task_id: val
                .get("id")
                .or_else(|| val.get("task_id"))
                .and_then(|v| v.as_str())
                .unwrap_or(&task.task_id)
                .to_string(),
            state: AgentTaskState::Submitted,
            debugging_result: None,
            raw_payload: Some(val),
        })
    }

    async fn get_status(&self, task_id: &str) -> Result<AgentTaskStatus> {
        let url = format!("{}/tasks/{task_id}", self.endpoint.trim_end_matches('/'));
        let mut req = self.client.get(&url);
        if let Some(tok) = &self.bearer_token {
            req = req.bearer_auth(tok);
        }
        let resp = req.send().await.map_err(|e| BugHunterError::Provider {
            provider: self.name.clone(),
            message: e.to_string(),
            retryable: true,
            status_code: None,
        })?;
        let val: serde_json::Value = resp.json().await.unwrap_or_default();
        let state_str = val
            .get("state")
            .or_else(|| val.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("working");
        let state = match state_str {
            "completed" => AgentTaskState::Completed,
            "failed" => AgentTaskState::Failed,
            "cancelled" | "canceled" => AgentTaskState::Cancelled,
            "submitted" | "pending" => AgentTaskState::Submitted,
            _ => AgentTaskState::Working,
        };
        Ok(AgentTaskStatus {
            task_id: task_id.to_string(),
            state,
            message: val.get("message").and_then(|v| v.as_str()).map(String::from),
        })
    }
}

/// Registry of supported agent adapters.
pub struct AgentRegistry {
    adapters: BTreeMap<String, Arc<dyn AgentAdapter>>,
}

impl AgentRegistry {
    pub fn with_builtin_adapters(core: Arc<BugHunterCore>, auth: AuthContext) -> Self {
        let mut adapters: BTreeMap<String, Arc<dyn AgentAdapter>> = BTreeMap::new();
        let kinds = [
            AgentFrameworkKind::OpenClaw,
            AgentFrameworkKind::HermesAgent,
            AgentFrameworkKind::OpenCode,
            AgentFrameworkKind::ClaudeCode,
            AgentFrameworkKind::OpenHands,
            AgentFrameworkKind::SweAgent,
            AgentFrameworkKind::LangGraph,
            AgentFrameworkKind::CrewAi,
            AgentFrameworkKind::AutoGen,
            AgentFrameworkKind::GoogleAdk,
            AgentFrameworkKind::OpenAiAgentsSdk,
            AgentFrameworkKind::Custom("custom".to_string()),
        ];

        for kind in kinds {
            let id = kind.id();
            adapters.insert(
                id.clone(),
                Arc::new(CoreBackedAgentAdapter::new(
                    id,
                    kind,
                    Arc::clone(&core),
                    auth.clone(),
                )),
            );
        }

        Self { adapters }
    }

    pub fn get(&self, id: &str) -> Result<Arc<dyn AgentAdapter>> {
        self.adapters
            .get(id)
            .cloned()
            .ok_or_else(|| BugHunterError::NotFound(format!("agent adapter '{id}' not found")))
    }

    pub async fn list_capabilities(&self) -> Vec<AgentCapabilities> {
        let mut out = Vec::with_capacity(self.adapters.len());
        for adapter in self.adapters.values() {
            if let Ok(caps) = adapter.discover().await {
                out.push(caps);
            }
        }
        out
    }
}
