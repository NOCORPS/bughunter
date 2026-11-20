# Model Provider System

The `bughunter-providers` crate defines a provider-independent abstraction in [types.rs](../crates/bughunter-providers/src/types.rs):

```rust
#[async_trait]
pub trait ModelProvider: Send + Sync {
    async fn generate(&self, request: ModelRequest) -> Result<ModelResponse>;
    async fn stream(&self, request: ModelRequest) -> Result<ModelStream>;
    async fn capabilities(&self) -> Result<ModelCapabilities>;
    async fn discover_models(&self) -> Result<Vec<ModelInfo>>;
}
```

## Built-in Providers (15 Presets)

`ProviderRegistry::with_builtin_presets()` ([registry.rs](../crates/bughunter-providers/src/registry.rs)) registers all 15 built-in adapters from environment variables:

| Registry Name | Implementation | Default Base URL | Env Var | Default Model | Tools | Structured Output | Discovery |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `openai` | OpenAI-compatible | `https://api.openai.com/v1` | `OPENAI_API_KEY` | `gpt-4o` | Yes | Yes | Yes |
| `anthropic` | Native Messages API | `https://api.anthropic.com` | `ANTHROPIC_API_KEY` | `claude-3-7-sonnet-latest` | Yes | Fallback | Static |
| `gemini` | Native Gemini API | `https://generativelanguage.googleapis.com/v1beta` | `GEMINI_API_KEY` | `gemini-2.5-flash` | Yes | Yes | Yes |
| `xai` | OpenAI-compatible | `https://api.x.ai/v1` | `XAI_API_KEY` | `grok-2-latest` | Yes | Yes | Yes |
| `deepseek` | OpenAI-compatible | `https://api.deepseek.com/v1` | `DEEPSEEK_API_KEY` | `deepseek-chat` | Yes | Yes | Yes |
| `mistral` | OpenAI-compatible | `https://api.mistral.ai/v1` | `MISTRAL_API_KEY` | `mistral-large-latest` | Yes | Yes | Yes |
| `meta_llama` | OpenAI-compatible | `https://api.together.xyz/v1` | `TOGETHER_API_KEY` | `meta-llama/Llama-3.3-70B-Instruct-Turbo` | Yes | Fallback | Yes |
| `qwen` | OpenAI-compatible | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` | `DASHSCOPE_API_KEY` | `qwen-plus` | Yes | Yes | Yes |
| `ollama` | OpenAI-compatible | `http://127.0.0.1:11434/v1` | `OLLAMA_API_KEY` | `llama3.1` | Yes | Yes | Yes |
| `lm_studio` | OpenAI-compatible | `http://127.0.0.1:1234/v1` | `LM_STUDIO_API_KEY` | `local-model` | Fallback | Fallback | Yes |
| `openrouter` | OpenAI-compatible | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` | `openai/gpt-4o` | Yes | Yes | Yes |
| `together_ai` | OpenAI-compatible | `https://api.together.xyz/v1` | `TOGETHER_API_KEY` | `meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo` | Yes | Fallback | Yes |
| `fireworks_ai` | OpenAI-compatible | `https://api.fireworks.ai/inference/v1` | `FIREWORKS_API_KEY` | `accounts/fireworks/models/llama-v3p1-70b-instruct` | Yes | Yes | Yes |
| `groq` | OpenAI-compatible | `https://api.groq.com/openai/v1` | `GROQ_API_KEY` | `llama-3.3-70b-versatile` | Yes | Fallback | Yes |
| `custom_openai` | OpenAI-compatible | `http://127.0.0.1:8000/v1` | `CUSTOM_OPENAI_API_KEY` | `default` | Fallback | Fallback | Yes |

## Reliability & Capability Fallbacks

Implemented in [http.rs](../crates/bughunter-providers/src/http.rs):

- **Timeouts & Retries**: Configurable via `ProviderRuntimeConfig` (`timeout_ms: 30_000`, `max_retries: 2`, `initial_backoff_ms: 250`). Retries on timeouts, connection errors, HTTP `429` (respecting `Retry-After` headers), and `5xx` responses with exponential backoff.
- **Error Normalization**: HTTP `401`/`403` -> `BugHunterError::Authentication`, `429` -> `BugHunterError::RateLimited`, `400`/`422` -> `BugHunterError::Validation`, others -> `BugHunterError::Provider`.
- **Context-Window Awareness**: `prepare_request_with_fallbacks` checks `ModelRequest::estimated_input_tokens()` against `ModelCapabilities::max_context_tokens` before sending.
- **Graceful Fallbacks**:
  - When `supports_structured_output` is `false`, `response_schema` is converted into a system JSON instruction instead of failing.
  - When `supports_tool_calling` is `false`, tool definitions are converted into a system capability note.
