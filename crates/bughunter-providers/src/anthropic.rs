use crate::http::{prepare_request_with_fallbacks, send_with_retries};
use crate::types::{
    MessageRole, ModelCapabilities, ModelProvider, ModelRequest, ModelResponse, ModelStream,
    ModelStreamChunk, ProviderRuntimeConfig, TokenUsage, ToolCall,
};
use async_trait::async_trait;
use bughunter_core::{BugHunterError, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    pub base_url: String,
    pub api_key: String,
    pub api_version: String,
    pub default_model: String,
    pub max_context_tokens: usize,
    pub runtime: ProviderRuntimeConfig,
}

impl AnthropicConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            base_url: "https://api.anthropic.com".to_string(),
            api_key: api_key.into(),
            api_version: "2023-06-01".to_string(),
            default_model: "claude-3-7-sonnet-latest".to_string(),
            max_context_tokens: 200_000,
            runtime: ProviderRuntimeConfig::default(),
        }
    }
}

pub struct AnthropicProvider {
    config: AnthropicConfig,
    client: reqwest::Client,
}

impl AnthropicProvider {
    pub fn new(config: AnthropicConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    fn build_payload(&self, req: &ModelRequest, stream: bool) -> serde_json::Value {
        let mut system_parts = Vec::new();
        let mut messages = Vec::new();

        for m in &req.messages {
            match m.role {
                MessageRole::System => system_parts.push(m.content.clone()),
                MessageRole::User | MessageRole::Tool => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": m.content,
                    }));
                }
                MessageRole::Assistant => {
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": m.content,
                    }));
                }
            }
        }

        let model = if req.model.trim().is_empty() {
            self.config.default_model.clone()
        } else {
            req.model.clone()
        };

        let mut payload = serde_json::json!({
            "model": model,
            "max_tokens": req.max_tokens.unwrap_or(4096),
            "messages": messages,
            "stream": stream,
        });
        if !system_parts.is_empty() {
            payload["system"] = serde_json::json!(system_parts.join("\n"));
        }
        if let Some(temp) = req.temperature {
            payload["temperature"] = serde_json::json!(temp);
        }
        if !req.tools.is_empty() {
            let tools: Vec<serde_json::Value> = req
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.parameters,
                    })
                })
                .collect();
            payload["tools"] = serde_json::json!(tools);
        }
        payload
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        let caps = self.capabilities().await?;
        let prepared = prepare_request_with_fallbacks(request, &caps)?;
        let payload = self.build_payload(&prepared, false);
        let url = format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'));
        let api_key = self.config.api_key.clone();
        let api_version = self.config.api_version.clone();
        let client = self.client.clone();

        let resp = send_with_retries("anthropic", &self.config.runtime, || {
            client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", &api_version)
                .json(&payload)
                .send()
        })
        .await?;

        let json: serde_json::Value = resp.json().await.map_err(|e| BugHunterError::Provider {
            provider: "anthropic".to_string(),
            message: format!("invalid JSON response: {e}"),
            retryable: false,
            status_code: None,
        })?;

        let mut text_content = String::new();
        let mut tool_calls = Vec::new();
        if let Some(blocks) = json.get("content").and_then(|c| c.as_array()) {
            for block in blocks {
                match block.get("type").and_then(|t| t.as_str()) {
                    Some("text") => {
                        if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                            text_content.push_str(t);
                        }
                    }
                    Some("tool_use") => {
                        if let (Some(id), Some(name)) = (
                            block.get("id").and_then(|v| v.as_str()),
                            block.get("name").and_then(|v| v.as_str()),
                        ) {
                            tool_calls.push(ToolCall {
                                id: id.to_string(),
                                name: name.to_string(),
                                arguments: block.get("input").cloned().unwrap_or_default(),
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        let usage_obj = json.get("usage");
        let input_tokens = usage_obj
            .and_then(|u| u.get("input_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output_tokens = usage_obj
            .and_then(|u| u.get("output_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(ModelResponse {
            provider: "anthropic".to_string(),
            model: json
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or(&self.config.default_model)
                .to_string(),
            content: text_content.clone(),
            tool_calls,
            usage: TokenUsage {
                prompt_tokens: input_tokens,
                completion_tokens: output_tokens,
                total_tokens: input_tokens + output_tokens,
            },
            finish_reason: json
                .get("stop_reason")
                .and_then(|s| s.as_str())
                .map(String::from),
            structured_output: serde_json::from_str(&text_content).ok(),
        })
    }

    async fn stream(&self, request: ModelRequest) -> Result<ModelStream> {
        let caps = self.capabilities().await?;
        let prepared = prepare_request_with_fallbacks(request, &caps)?;
        let payload = self.build_payload(&prepared, true);
        let url = format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'));
        let api_key = self.config.api_key.clone();
        let api_version = self.config.api_version.clone();
        let client = self.client.clone();

        let resp = send_with_retries("anthropic", &self.config.runtime, || {
            client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", &api_version)
                .json(&payload)
                .send()
        })
        .await?;

        let stream = resp.bytes_stream().map(|chunk_res| match chunk_res {
            Err(e) => Err(BugHunterError::Provider {
                provider: "anthropic".to_string(),
                message: e.to_string(),
                retryable: false,
                status_code: None,
            }),
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                let mut delta = String::new();
                let mut done = false;
                for line in text.lines() {
                    if let Some(data) = line.trim().strip_prefix("data: ") {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                            if val.get("type").and_then(|t| t.as_str()) == Some("message_stop") {
                                done = true;
                            }
                            if let Some(t) = val
                                .get("delta")
                                .and_then(|d| d.get("text"))
                                .and_then(|s| s.as_str())
                            {
                                delta.push_str(t);
                            }
                        }
                    }
                }
                Ok(ModelStreamChunk {
                    delta,
                    done,
                    usage: None,
                    finish_reason: None,
                })
            }
        });

        Ok(Box::pin(stream))
    }

    async fn capabilities(&self) -> Result<ModelCapabilities> {
        Ok(ModelCapabilities {
            provider: "anthropic".to_string(),
            supports_streaming: true,
            supports_tool_calling: true,
            supports_structured_output: false, // Uses graceful JSON prompt fallback
            supports_model_discovery: false,
            max_context_tokens: self.config.max_context_tokens,
            default_model: self.config.default_model.clone(),
        })
    }
}
