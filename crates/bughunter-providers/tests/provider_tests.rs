use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use bughunter_core::BugHunterError;
use bughunter_providers::{
    AnthropicConfig, AnthropicProvider, ChatMessage, GeminiConfig, GeminiProvider, ModelProvider,
    ModelRequest, OpenAiCompatibleConfig, OpenAiCompatiblePreset, OpenAiCompatibleProvider,
    ProviderRegistry, ProviderRuntimeConfig, StructuredOutputSchema, ToolDefinition,
};
use futures::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

async fn spawn_mock_server(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn test_openai_compatible_generate_stream_discover_and_fallback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route(
            "/v1/chat/completions",
            post({
                let calls = Arc::clone(&calls);
                move |headers: HeaderMap, Json(body): Json<serde_json::Value>| async move {
                    let count = calls.fetch_add(1, Ordering::SeqCst);
                    if headers.get("authorization").and_then(|v| v.to_str().ok())
                        == Some("Bearer bad-key")
                    {
                        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "bad key"})))
                            .into_response();
                    }
                    if body.get("stream").and_then(|v| v.as_bool()) == Some(true) {
                        let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"hel\"}}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\ndata: [DONE]\n\n";
                        return (
                            StatusCode::OK,
                            [("content-type", "text/event-stream")],
                            sse.to_string(),
                        )
                            .into_response();
                    }
                    if count == 1 {
                        return (
                            StatusCode::TOO_MANY_REQUESTS,
                            [("retry-after", "0")],
                            Json(serde_json::json!({"error": "rate limit"})),
                        )
                            .into_response();
                    }
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "model": "gpt-4o",
                            "choices": [{
                                "message": {
                                    "role": "assistant",
                                    "content": "{\"ok\":true}",
                                    "tool_calls": [{
                                        "id": "tc_1",
                                        "type": "function",
                                        "function": { "name": "inspect", "arguments": "{\"path\":\"src/lib.rs\"}" }
                                    }]
                                },
                                "finish_reason": "stop"
                            }],
                            "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
                        })),
                    )
                        .into_response()
                }
            }),
        )
        .route(
            "/v1/models",
            get(|| async {
                Json(serde_json::json!({
                    "data": [{ "id": "gpt-4o" }, { "id": "o3-mini" }]
                }))
            }),
        );

    let base = spawn_mock_server(app).await;

    let mut cfg = OpenAiCompatibleConfig::preset(OpenAiCompatiblePreset::OpenAi, Some("good-key".into()));
    cfg.base_url = format!("{base}/v1");
    cfg.runtime = ProviderRuntimeConfig {
        timeout_ms: 5000,
        max_retries: 2,
        initial_backoff_ms: 10,
    };
    let provider = OpenAiCompatibleProvider::new(cfg.clone());

    let mut req = ModelRequest::new("gpt-4o", vec![ChatMessage::user("hello")]);
    req.response_schema = Some(StructuredOutputSchema {
        name: "result".into(),
        schema: serde_json::json!({"type": "object"}),
        strict: true,
    });
    req.tools.push(ToolDefinition {
        name: "inspect".into(),
        description: "inspect file".into(),
        parameters: serde_json::json!({"type": "object"}),
    });

    // First generate succeeds
    let resp = provider.generate(req.clone()).await.unwrap();
    assert_eq!(resp.provider, "openai");
    assert_eq!(resp.usage.total_tokens, 15);
    assert_eq!(resp.tool_calls.len(), 1);
    assert_eq!(resp.structured_output.unwrap()["ok"], true);

    // Second generate hits 429 once then retries and succeeds
    let resp2 = provider.generate(req.clone()).await.unwrap();
    assert_eq!(resp2.content, "{\"ok\":true}");

    // Streaming
    let mut stream = provider.stream(req).await.unwrap();
    let mut streamed = String::new();
    while let Some(chunk) = stream.next().await {
        streamed.push_str(&chunk.unwrap().delta);
    }
    assert_eq!(streamed, "hello");

    // Discovery
    let models = provider.discover_models().await.unwrap();
    assert_eq!(models.len(), 2);

    // Auth error normalization
    cfg.api_key = Some("bad-key".into());
    let bad_provider = OpenAiCompatibleProvider::new(cfg);
    let err = bad_provider
        .generate(ModelRequest::new("gpt-4o", vec![ChatMessage::user("hi")]))
        .await
        .unwrap_err();
    assert!(matches!(err, BugHunterError::Authentication(_)));
}

#[tokio::test]
async fn test_anthropic_provider_mock() {
    let app = Router::new().route(
        "/v1/messages",
        post(|Json(body): Json<serde_json::Value>| async move {
            if body.get("stream").and_then(|v| v.as_bool()) == Some(true) {
                let sse = "data: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"ok\"}}\n\ndata: {\"type\":\"message_stop\"}\n\n";
                return (
                    StatusCode::OK,
                    [("content-type", "text/event-stream")],
                    sse.to_string(),
                )
                    .into_response();
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "model": "claude-3-7-sonnet-latest",
                    "stop_reason": "end_turn",
                    "content": [{ "type": "text", "text": "anthropic response" }],
                    "usage": { "input_tokens": 12, "output_tokens": 8 }
                })),
            )
                .into_response()
        }),
    );

    let base = spawn_mock_server(app).await;
    let mut cfg = AnthropicConfig::new("test-key");
    cfg.base_url = base;
    let provider = AnthropicProvider::new(cfg);

    let caps = provider.capabilities().await.unwrap();
    assert_eq!(caps.provider, "anthropic");
    assert!(!caps.supports_structured_output);

    let resp = provider
        .generate(ModelRequest::new("", vec![ChatMessage::user("debug")]))
        .await
        .unwrap();
    assert_eq!(resp.content, "anthropic response");
    assert_eq!(resp.usage.total_tokens, 20);

    let mut stream = provider
        .stream(ModelRequest::new("", vec![ChatMessage::user("debug")]))
        .await
        .unwrap();
    let mut text = String::new();
    while let Some(c) = stream.next().await {
        text.push_str(&c.unwrap().delta);
    }
    assert_eq!(text, "ok");
}

#[tokio::test]
async fn test_gemini_provider_mock_and_registry() {
    let app = Router::new()
        .route(
            "/v1beta/models/gemini-2.5-flash:generateContent",
            post(|| async {
                Json(serde_json::json!({
                    "candidates": [{
                        "content": { "parts": [{ "text": "gemini response" }] },
                        "finishReason": "STOP"
                    }],
                    "usageMetadata": {
                        "promptTokenCount": 5,
                        "candidatesTokenCount": 4,
                        "totalTokenCount": 9
                    }
                }))
            }),
        )
        .route(
            "/v1beta/models",
            get(|| async {
                Json(serde_json::json!({
                    "models": [{ "name": "models/gemini-2.5-flash", "inputTokenLimit": 1000000 }]
                }))
            }),
        );

    let base = spawn_mock_server(app).await;
    let mut cfg = GeminiConfig::new("test-key");
    cfg.base_url = format!("{base}/v1beta");
    let provider = GeminiProvider::new(cfg);

    let resp = provider
        .generate(ModelRequest::new("", vec![ChatMessage::user("hi")]))
        .await
        .unwrap();
    assert_eq!(resp.content, "gemini response");
    assert_eq!(resp.usage.total_tokens, 9);

    let models = provider.discover_models().await.unwrap();
    assert_eq!(models[0].id, "models/gemini-2.5-flash");

    // Verify all 15 built-in providers are registered
    let reg = ProviderRegistry::with_builtin_presets();
    let names = reg.names();
    for expected in [
        "openai",
        "anthropic",
        "gemini",
        "xai",
        "deepseek",
        "mistral",
        "meta_llama",
        "qwen",
        "ollama",
        "lm_studio",
        "openrouter",
        "together_ai",
        "fireworks_ai",
        "groq",
        "custom_openai",
    ] {
        assert!(names.contains(&expected.to_string()), "missing {expected}");
    }
}
