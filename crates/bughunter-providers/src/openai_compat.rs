use crate::http::{prepare_request_with_fallbacks, send_with_retries};
use crate::types::{
    MessageRole, ModelCapabilities, ModelInfo, ModelProvider, ModelRequest, ModelResponse,
    ModelStream, ModelStreamChunk, ProviderRuntimeConfig, TokenUsage, ToolCall,
};
use async_trait::async_trait;
use bughunter_core::{BugHunterError, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenAiCompatiblePreset {
    OpenAi,
    Xai,
    DeepSeek,
    Mistral,
    MetaLlama,
    Qwen,
    Ollama,
    LmStudio,
    OpenRouter,
    TogetherAi,
    FireworksAi,
    Groq,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiCompatibleConfig {
    pub name: String,
    pub preset: OpenAiCompatiblePreset,
    pub base_url: String,
    pub api_key: Option<String>,
    pub default_model: String,
    pub max_context_tokens: usize,
    pub supports_structured_output: bool,
    pub supports_tool_calling: bool,
    pub supports_model_discovery: bool,
    pub runtime: ProviderRuntimeConfig,
}

impl OpenAiCompatibleConfig {
    pub fn preset(preset: OpenAiCompatiblePreset, api_key: Option<String>) -> Self {
        match preset {
            OpenAiCompatiblePreset::OpenAi => Self {
                name: "openai".to_string(),
                preset,
                base_url: "https://api.openai.com/v1".to_string(),
                api_key,
                default_model: "gpt-4o".to_string(),
                max_context_tokens: 128_000,
                supports_structured_output: true,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::Xai => Self {
                name: "xai".to_string(),
                preset,
                base_url: "https://api.x.ai/v1".to_string(),
                api_key,
                default_model: "grok-2-latest".to_string(),
                max_context_tokens: 131_072,
                supports_structured_output: true,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::DeepSeek => Self {
                name: "deepseek".to_string(),
                preset,
                base_url: "https://api.deepseek.com/v1".to_string(),
                api_key,
                default_model: "deepseek-chat".to_string(),
                max_context_tokens: 64_000,
                supports_structured_output: true,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::Mistral => Self {
                name: "mistral".to_string(),
                preset,
                base_url: "https://api.mistral.ai/v1".to_string(),
                api_key,
                default_model: "mistral-large-latest".to_string(),
                max_context_tokens: 128_000,
                supports_structured_output: true,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::MetaLlama => Self {
                name: "meta_llama".to_string(),
                preset,
                base_url: "https://api.together.xyz/v1".to_string(),
                api_key,
                default_model: "meta-llama/Llama-3.3-70B-Instruct-Turbo".to_string(),
                max_context_tokens: 128_000,
                supports_structured_output: false,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::Qwen => Self {
                name: "qwen".to_string(),
                preset,
                base_url: "https://dashscope-intl.aliyuncs.com/compatible-mode/v1".to_string(),
                api_key,
                default_model: "qwen-plus".to_string(),
                max_context_tokens: 131_072,
                supports_structured_output: true,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::Ollama => Self {
                name: "ollama".to_string(),
                preset,
                base_url: "http://127.0.0.1:11434/v1".to_string(),
                api_key,
                default_model: "llama3.1".to_string(),
                max_context_tokens: 32_768,
                supports_structured_output: true,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::LmStudio => Self {
                name: "lm_studio".to_string(),
                preset,
                base_url: "http://127.0.0.1:1234/v1".to_string(),
                api_key,
                default_model: "local-model".to_string(),
                max_context_tokens: 32_768,
                supports_structured_output: false,
                supports_tool_calling: false,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::OpenRouter => Self {
                name: "openrouter".to_string(),
                preset,
                base_url: "https://openrouter.ai/api/v1".to_string(),
                api_key,
                default_model: "openai/gpt-4o".to_string(),
                max_context_tokens: 128_000,
                supports_structured_output: true,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::TogetherAi => Self {
                name: "together_ai".to_string(),
                preset,
                base_url: "https://api.together.xyz/v1".to_string(),
                api_key,
                default_model: "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo".to_string(),
                max_context_tokens: 128_000,
                supports_structured_output: false,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::FireworksAi => Self {
                name: "fireworks_ai".to_string(),
                preset,
                base_url: "https://api.fireworks.ai/inference/v1".to_string(),
                api_key,
                default_model: "accounts/fireworks/models/llama-v3p1-70b-instruct".to_string(),
                max_context_tokens: 131_072,
                supports_structured_output: true,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::Groq => Self {
                name: "groq".to_string(),
                preset,
                base_url: "https://api.groq.com/openai/v1".to_string(),
                api_key,
                default_model: "llama-3.3-70b-versatile".to_string(),
                max_context_tokens: 128_000,
                supports_structured_output: false,
                supports_tool_calling: true,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
            OpenAiCompatiblePreset::Custom => Self {
                name: "custom_openai".to_string(),
                preset,
                base_url: "http://127.0.0.1:8000/v1".to_string(),
                api_key,
                default_model: "default".to_string(),
                max_context_tokens: 32_768,
                supports_structured_output: false,
                supports_tool_calling: false,
                supports_model_discovery: true,
                runtime: ProviderRuntimeConfig::default(),
            },
        }
    }
}

pub struct OpenAiCompatibleProvider {
    config: OpenAiCompatibleConfig,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: OpenAiCompatibleConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    fn build_payload(&self, req: &ModelRequest, stream: bool) -> serde_json::Value {
        let model = if req.model.trim().is_empty() {
            self.config.default_model.clone()
        } else {
            req.model.clone()
        };
        let messages: Vec<serde_json::Value> = req
            .messages
            .iter()
            .map(|m| {
                let role = match m.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Tool => "tool",
                };
                let mut obj = serde_json::json!({
                    "role": role,
                    "content": m.content,
                });
                if let Some(id) = &m.tool_call_id {
                    obj["tool_call_id"] = serde_json::json!(id);
                }
                if let Some(name) = &m.name {
                    obj["name"] = serde_json::json!(name);
                }
                obj
            })
            .collect();

        let mut payload = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": stream,
        });
        if let Some(temp) = req.temperature {
            payload["temperature"] = serde_json::json!(temp);
        }
        if let Some(max_tokens) = req.max_tokens {
            payload["max_tokens"] = serde_json::json!(max_tokens);
        }
        if !req.tools.is_empty() {
            let tools: Vec<serde_json::Value> = req
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            payload["tools"] = serde_json::json!(tools);
        }
        if let Some(schema) = &req.response_schema {
            payload["response_format"] = serde_json::json!({
                "type": "json_schema",
                "json_schema": {
                    "name": schema.name,
                    "schema": schema.schema,
                    "strict": schema.strict,
                }
            });
        }
        payload
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        let caps = self.capabilities().await?;
        let prepared = prepare_request_with_fallbacks(request, &caps)?;
        let payload = self.build_payload(&prepared, false);
        let url = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));
        let api_key = self.config.api_key.clone();
        let client = self.client.clone();

        let resp = send_with_retries(&self.config.name, &self.config.runtime, || {
            let mut builder = client.post(&url).json(&payload);
            if let Some(key) = &api_key {
                builder = builder.bearer_auth(key);
            }
            builder.send()
        })
        .await?;

        let json: serde_json::Value = resp.json().await.map_err(|e| BugHunterError::Provider {
            provider: self.config.name.clone(),
            message: format!("invalid JSON response: {e}"),
            retryable: false,
            status_code: None,
        })?;

        let choice = json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| BugHunterError::Provider {
                provider: self.config.name.clone(),
                message: "missing choices in response".to_string(),
                retryable: false,
                status_code: None,
            })?;

        let message = choice.get("message").cloned().unwrap_or_default();
        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .unwrap_or_default()
            .to_string();

        let tool_calls = message
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let id = item.get("id")?.as_str()?.to_string();
                        let func = item.get("function")?;
                        let name = func.get("name")?.as_str()?.to_string();
                        let args_str = func.get("arguments").and_then(|a| a.as_str()).unwrap_or("{}");
                        let arguments = serde_json::from_str(args_str)
                            .unwrap_or_else(|_| serde_json::json!({ "raw": args_str }));
                        Some(ToolCall {
                            id,
                            name,
                            arguments,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let usage_obj = json.get("usage");
        let usage = TokenUsage {
            prompt_tokens: usage_obj
                .and_then(|u| u.get("prompt_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            completion_tokens: usage_obj
                .and_then(|u| u.get("completion_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            total_tokens: usage_obj
                .and_then(|u| u.get("total_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        };

        let structured_output = if prepared.response_schema.is_some() {
            serde_json::from_str(&content).ok()
        } else {
            None
        };

        Ok(ModelResponse {
            provider: self.config.name.clone(),
            model: json
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or(&self.config.default_model)
                .to_string(),
            content,
            tool_calls,
            usage,
            finish_reason: choice
                .get("finish_reason")
                .and_then(|f| f.as_str())
                .map(String::from),
            structured_output,
        })
    }

    async fn stream(&self, request: ModelRequest) -> Result<ModelStream> {
        let caps = self.capabilities().await?;
        let prepared = prepare_request_with_fallbacks(request, &caps)?;
        let payload = self.build_payload(&prepared, true);
        let url = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));
        let api_key = self.config.api_key.clone();
        let client = self.client.clone();
        let provider_name = self.config.name.clone();

        let resp = send_with_retries(&self.config.name, &self.config.runtime, || {
            let mut builder = client.post(&url).json(&payload);
            if let Some(key) = &api_key {
                builder = builder.bearer_auth(key);
            }
            builder.send()
        })
        .await?;

        let stream = resp.bytes_stream().map(move |chunk_res| match chunk_res {
            Err(e) => Err(BugHunterError::Provider {
                provider: provider_name.clone(),
                message: e.to_string(),
                retryable: false,
                status_code: None,
            }),
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                let mut combined_delta = String::new();
                let mut done = false;
                for line in text.lines() {
                    let line = line.trim();
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            done = true;
                            continue;
                        }
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(delta) = val
                                .get("choices")
                                .and_then(|c| c.as_array())
                                .and_then(|a| a.first())
                                .and_then(|c| c.get("delta"))
                                .and_then(|d| d.get("content"))
                                .and_then(|s| s.as_str())
                            {
                                combined_delta.push_str(delta);
                            }
                        }
                    }
                }
                Ok(ModelStreamChunk {
                    delta: combined_delta,
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
            provider: self.config.name.clone(),
            supports_streaming: true,
            supports_tool_calling: self.config.supports_tool_calling,
            supports_structured_output: self.config.supports_structured_output,
            supports_model_discovery: self.config.supports_model_discovery,
            max_context_tokens: self.config.max_context_tokens,
            default_model: self.config.default_model.clone(),
        })
    }

    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        if !self.config.supports_model_discovery {
            let caps = self.capabilities().await?;
            return Ok(vec![ModelInfo {
                id: caps.default_model,
                provider: caps.provider,
                context_window: Some(caps.max_context_tokens),
            }]);
        }
        let url = format!("{}/models", self.config.base_url.trim_end_matches('/'));
        let api_key = self.config.api_key.clone();
        let client = self.client.clone();

        let resp = send_with_retries(&self.config.name, &self.config.runtime, || {
            let mut builder = client.get(&url);
            if let Some(key) = &api_key {
                builder = builder.bearer_auth(key);
            }
            builder.send()
        })
        .await?;

        let json: serde_json::Value = resp.json().await.map_err(|e| BugHunterError::Provider {
            provider: self.config.name.clone(),
            message: format!("invalid model list JSON: {e}"),
            retryable: false,
            status_code: None,
        })?;

        let models = json
            .get("data")
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let id = m.get("id")?.as_str()?.to_string();
                        Some(ModelInfo {
                            id,
                            provider: self.config.name.clone(),
                            context_window: Some(self.config.max_context_tokens),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }
}
