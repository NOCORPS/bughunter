use async_trait::async_trait;
use bughunter_core::Result;
use futures::Stream;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::pin::Pin;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_call_id: None,
            name: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredOutputSchema {
    pub name: String,
    pub schema: serde_json::Value,
    #[serde(default)]
    pub strict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<StructuredOutputSchema>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl ModelRequest {
    pub fn new(model: impl Into<String>, messages: Vec<ChatMessage>) -> Self {
        Self {
            model: model.into(),
            messages,
            temperature: None,
            max_tokens: None,
            tools: Vec::new(),
            response_schema: None,
            metadata: BTreeMap::new(),
        }
    }

    /// Rough token estimate (~4 chars per token) for context-window awareness.
    pub fn estimated_input_tokens(&self) -> usize {
        self.messages
            .iter()
            .map(|m| (m.content.len() / 4).max(1) + 4)
            .sum()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub provider: String,
    pub model: String,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    pub usage: TokenUsage,
    pub finish_reason: Option<String>,
    pub structured_output: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStreamChunk {
    pub delta: String,
    pub done: bool,
    pub usage: Option<TokenUsage>,
    pub finish_reason: Option<String>,
}

pub type ModelStream = Pin<Box<dyn Stream<Item = Result<ModelStreamChunk>> + Send>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub provider: String,
    pub supports_streaming: bool,
    pub supports_tool_calling: bool,
    pub supports_structured_output: bool,
    pub supports_model_discovery: bool,
    pub max_context_tokens: usize,
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub context_window: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRuntimeConfig {
    pub timeout_ms: u64,
    pub max_retries: u32,
    pub initial_backoff_ms: u64,
}

impl Default for ProviderRuntimeConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 30_000,
            max_retries: 2,
            initial_backoff_ms: 250,
        }
    }
}

#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse>;

    async fn stream(&self, request: ModelRequest) -> Result<ModelStream>;

    async fn capabilities(&self) -> Result<ModelCapabilities>;

    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let caps = self.capabilities().await?;
        Ok(vec![ModelInfo {
            id: caps.default_model,
            provider: caps.provider,
            context_window: Some(caps.max_context_tokens),
        }])
    }
}
