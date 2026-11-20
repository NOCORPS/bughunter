use bughunter_core::{DebuggingResult, InvestigationStatus};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub url: String,
    pub version: String,
    pub capabilities: AgentCardCapabilities,
    pub authentication: Vec<String>,
    pub skills: Vec<AgentCardSkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardCapabilities {
    pub streaming: bool,
    pub push_notifications: bool,
    pub state_transition_history: bool,
    pub async_tasks: bool,
    pub cancellation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskRequest {
    #[serde(default = "default_capability")]
    pub capability: String,
    pub repository: String,
    pub issue: Option<String>,
    #[serde(default)]
    pub verification_required: bool,
    #[serde(default)]
    pub files: Vec<String>,
    pub patch: Option<String>,
}

fn default_capability() -> String {
    "debugging".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2aTaskResponse {
    pub task_id: String,
    pub status: InvestigationStatus,
    pub result: Option<DebuggingResult>,
    pub error: Option<String>,
}
