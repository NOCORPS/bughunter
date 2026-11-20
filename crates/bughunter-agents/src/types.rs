use async_trait::async_trait;
use bughunter_core::{DebuggingRequest, DebuggingResult, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentProtocol {
    Mcp,
    A2a,
    HttpJson,
    Cli,
    CoreDirect,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentFrameworkKind {
    OpenClaw,
    HermesAgent,
    OpenCode,
    ClaudeCode,
    OpenHands,
    SweAgent,
    LangGraph,
    CrewAi,
    AutoGen,
    GoogleAdk,
    OpenAiAgentsSdk,
    Custom(String),
}

impl AgentFrameworkKind {
    pub fn id(&self) -> String {
        match self {
            Self::OpenClaw => "openclaw".to_string(),
            Self::HermesAgent => "hermes_agent".to_string(),
            Self::OpenCode => "opencode".to_string(),
            Self::ClaudeCode => "claude_code".to_string(),
            Self::OpenHands => "openhands".to_string(),
            Self::SweAgent => "swe_agent".to_string(),
            Self::LangGraph => "langgraph".to_string(),
            Self::CrewAi => "crewai".to_string(),
            Self::AutoGen => "autogen".to_string(),
            Self::GoogleAdk => "google_adk".to_string(),
            Self::OpenAiAgentsSdk => "openai_agents_sdk".to_string(),
            Self::Custom(name) => name.clone(),
        }
    }

    pub fn preferred_protocol(&self) -> AgentProtocol {
        match self {
            Self::OpenCode | Self::ClaudeCode | Self::OpenClaw | Self::HermesAgent | Self::OpenHands => {
                AgentProtocol::Mcp
            }
            Self::GoogleAdk | Self::LangGraph | Self::CrewAi | Self::AutoGen => AgentProtocol::A2a,
            Self::SweAgent | Self::OpenAiAgentsSdk | Self::Custom(_) => AgentProtocol::HttpJson,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilities {
    pub name: String,
    pub framework: AgentFrameworkKind,
    pub protocol: AgentProtocol,
    pub supports_async_tasks: bool,
    pub supports_cancellation: bool,
    pub supported_workflows: Vec<String>,
    pub endpoint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub task_id: String,
    pub request: DebuggingRequest,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskState {
    Submitted,
    Working,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskStatus {
    pub task_id: String,
    pub state: AgentTaskState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskResult {
    pub task_id: String,
    pub state: AgentTaskState,
    pub debugging_result: Option<DebuggingResult>,
    pub raw_payload: Option<serde_json::Value>,
}

#[async_trait]
pub trait AgentAdapter: Send + Sync {
    async fn discover(&self) -> Result<AgentCapabilities>;
    async fn submit_task(&self, task: AgentTask) -> Result<AgentTaskResult>;
    async fn get_status(&self, task_id: &str) -> Result<AgentTaskStatus>;
}
