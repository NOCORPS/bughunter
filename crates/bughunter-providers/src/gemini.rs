use crate::http::{prepare_request_with_fallbacks, send_with_retries};
use crate::types::{
    MessageRole, ModelCapabilities, ModelInfo, ModelProvider, ModelRequest, ModelResponse,
    ModelStream, ModelStreamChunk, ProviderRuntimeConfig, TokenUsage, ToolCall,
};
use async_trait::async_trait;
use bughunter_core::{BugHunterError, Result};
use futures::StreamExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConfig {
    pub base_url: String,
    pub api_key: String,
    pub default_model: String,
    pub max_context_tokens: usize,
    pub runtime: ProviderRuntimeConfig,
}

impl GeminiConfig {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            api_key: api_key.into(),
            default_model: "gemini-2.5-flash".to_string(),
            max_context_tokens: 1_000_000,
            runtime: ProviderRuntimeConfig::default(),
        }
    }
}

pub struct GeminiProvider {
    config: GeminiConfig,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(config: GeminiConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    fn build_payload(&self, req: &ModelRequest) -> serde_json::Value {
        let mut system_parts = Vec::new();
        let mut contents = Vec::new();

        for m in &req.messages {
            match m.role {
                MessageRole::System => system_parts.push(serde_json::json!({ "text": m.content })),
                MessageRole::User | MessageRole::Tool => contents.push(serde_json::json!({
                    "role": "user",
                    "parts": [{ "text": m.content }]
                })),
                MessageRole::Assistant => contents.push(serde_json::json!({
                    "role": "model",
                    "parts": [{ "text": m.content }]
                })),
            }
        }

        let mut payload = serde_json::json!({ "contents": contents });
        if !system_parts.is_empty() {
            payload["systemInstruction"] = serde_json::json!({ "parts": system_parts });
        }

        let mut gen_cfg = serde_json::Map::new();
        if let Some(temp) = req.temperature {
            gen_cfg.insert("temperature".to_string(), serde_json::json!(temp));
        }
        if let Some(max_tokens) = req.max_tokens {
            gen_cfg.insert("maxOutputTokens".to_string(), serde_json::json!(max_tokens));
        }
        if let Some(schema) = &req.response_schema {
            gen_cfg.insert(
                "responseMimeType".to_string(),
                serde_json::json!("application/json"),
            );
            gen_cfg.insert("responseSchema".to_string(), schema.schema.clone());
        }
        if !gen_cfg.is_empty() {
            payload["generationConfig"] = serde_json::Value::Object(gen_cfg);
        }

        if !req.tools.is_empty() {
            let decls: Vec<serde_json::Value> = req
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    })
                })
                .collect();
            payload["tools"] = serde_json::json!([{ "functionDeclarations": decls }]);
        }

        payload
    }
}

#[async_trait]
impl ModelProvider for GeminiProvider {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse> {
        let caps = self.capabilities().await?;
        let prepared = prepare_request_with_fallbacks(request, &caps)?;
        let model = if prepared.model.trim().is_empty() {
            self.config.default_model.clone()
        } else {
            prepared.model.clone()
        };
        let payload = self.build_payload(&prepared);
        let url = format!(
            "{}/models/{}:generateContent",
            self.config.base_url.trim_end_matches('/'),
            model
        );
        let api_key = self.config.api_key.clone();
        let client = self.client.clone();

        let resp = send_with_retries("gemini", &self.config.runtime, || {
            client
                .post(&url)
                .header("x-goog-api-key", &api_key)
                .json(&payload)
                .send()
        })
        .await?;

        let json: serde_json::Value = resp.json().await.map_err(|e| BugHunterError::Provider {
            provider: "gemini".to_string(),
            message: format!("invalid JSON response: {e}"),
            retryable: false,
            status_code: None,
        })?;

        let candidate = json
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| BugHunterError::Provider {
                provider: "gemini".to_string(),
                message: "missing candidates in response".to_string(),
                retryable: false,
                status_code: None,
            })?;

        let mut content = String::new();
        let mut tool_calls = Vec::new();
        if let Some(parts) = candidate
            .get("content")
            .and_then(|c| c.get("parts"))
            .and_then(|p| p.as_array())
        {
            for (idx, part) in parts.iter().enumerate() {
                if let Some(t) = part.get("text").and_then(|v| v.as_str()) {
                    content.push_str(t);
                }
                if let Some(fc) = part.get("functionCall") {
                    if let Some(name) = fc.get("name").and_then(|v| v.as_str()) {
                        tool_calls.push(ToolCall {
                            id: format!("call_{idx}"),
                            name: name.to_string(),
                            arguments: fc.get("args").cloned().unwrap_or_default(),
                        });
                    }
                }
            }
        }

        let usage_meta = json.get("usageMetadata");
        let prompt_tokens = usage_meta
            .and_then(|u| u.get("promptTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let completion_tokens = usage_meta
            .and_then(|u| u.get("candidatesTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let total_tokens = usage_meta
            .and_then(|u| u.get("totalTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(prompt_tokens + completion_tokens);

        let structured_output = if prepared.response_schema.is_some() {
            serde_json::from_str(&content).ok()
        } else {
            None
        };

        Ok(ModelResponse {
            provider: "gemini".to_string(),
            model,
            content,
            tool_calls,
            usage: TokenUsage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            },
            finish_reason: candidate
                .get("finishReason")
                .and_then(|f| f.as_str())
                .map(String::from),
            structured_output,
        })
    }

    async fn stream(&self, request: ModelRequest) -> Result<ModelStream> {
        let caps = self.capabilities().await?;
        let prepared = prepare_request_with_fallbacks(request, &caps)?;
        let model = if prepared.model.trim().is_empty() {
            self.config.default_model.clone()
        } else {
            prepared.model.clone()
        };
        let payload = self.build_payload(&prepared);
        let url = format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.config.base_url.trim_end_matches('/'),
            model
        );
        let api_key = self.config.api_key.clone();
        let client = self.client.clone();

        let resp = send_with_retries("gemini", &self.config.runtime, || {
            client
                .post(&url)
                .header("x-goog-api-key", &api_key)
                .json(&payload)
                .send()
        })
        .await?;

        let stream = resp.bytes_stream().map(|chunk_res| match chunk_res {
            Err(e) => Err(BugHunterError::Provider {
                provider: "gemini".to_string(),
                message: e.to_string(),
                retryable: false,
                status_code: None,
            }),
            Ok(bytes) => {
                let text = String::from_utf8_lossy(&bytes);
                let mut delta = String::new();
                for line in text.lines() {
                    if let Some(data) = line.trim().strip_prefix("data: ") {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(t) = val
                                .get("candidates")
                                .and_then(|c| c.as_array())
                                .and_then(|a| a.first())
                                .and_then(|c| c.get("content"))
                                .and_then(|c| c.get("parts"))
                                .and_then(|p| p.as_array())
                                .and_then(|a| a.first())
                                .and_then(|p| p.get("text"))
                                .and_then(|s| s.as_str())
                            {
                                delta.push_str(t);
                            }
                        }
                    }
                }
                Ok(ModelStreamChunk {
                    delta,
                    done: false,
                    usage: None,
                    finish_reason: None,
                })
            }
        });

        Ok(Box::pin(stream))
    }

    async fn capabilities(&self) -> Result<ModelCapabilities> {
        Ok(ModelCapabilities {
            provider: "gemini".to_string(),
            supports_streaming: true,
            supports_tool_calling: true,
            supports_structured_output: true,
            supports_model_discovery: true,
            max_context_tokens: self.config.max_context_tokens,
            default_model: self.config.default_model.clone(),
        })
    }

    async fn discover_models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/models", self.config.base_url.trim_end_matches('/'));
        let api_key = self.config.api_key.clone();
        let client = self.client.clone();

        let resp = send_with_retries("gemini", &self.config.runtime, || {
            client
                .get(&url)
                .header("x-goog-api-key", &api_key)
                .send()
        })
        .await?;

        let json: serde_json::Value = resp.json().await.map_err(|e| BugHunterError::Provider {
            provider: "gemini".to_string(),
            message: format!("invalid model list JSON: {e}"),
            retryable: false,
            status_code: None,
        })?;

        let models = json
            .get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| {
                        let name = m.get("name")?.as_str()?.to_string();
                        let ctx = m
                            .get("inputTokenLimit")
                            .and_then(|v| v.as_u64())
                            .map(|v| v as usize);
                        Some(ModelInfo {
                            id: name,
                            provider: "gemini".to_string(),
                            context_window: ctx,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(models)
    }
}
