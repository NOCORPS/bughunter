use crate::anthropic::{AnthropicConfig, AnthropicProvider};
use crate::gemini::{GeminiConfig, GeminiProvider};
use crate::openai_compat::{
    OpenAiCompatibleConfig, OpenAiCompatiblePreset, OpenAiCompatibleProvider,
};
use crate::types::{ModelCapabilities, ModelProvider};
use bughunter_core::{BugHunterError, Result};
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Default)]
pub struct ProviderRegistry {
    providers: BTreeMap<String, Arc<dyn ModelProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers all 15 built-in provider adapters from environment variables / defaults.
    /// No provider is mandatory; all are available for capability inspection and routing.
    pub fn with_builtin_presets() -> Self {
        let mut reg = Self::new();
        let presets = [
            (OpenAiCompatiblePreset::OpenAi, "OPENAI_API_KEY"),
            (OpenAiCompatiblePreset::Xai, "XAI_API_KEY"),
            (OpenAiCompatiblePreset::DeepSeek, "DEEPSEEK_API_KEY"),
            (OpenAiCompatiblePreset::Mistral, "MISTRAL_API_KEY"),
            (OpenAiCompatiblePreset::MetaLlama, "TOGETHER_API_KEY"),
            (OpenAiCompatiblePreset::Qwen, "DASHSCOPE_API_KEY"),
            (OpenAiCompatiblePreset::Ollama, "OLLAMA_API_KEY"),
            (OpenAiCompatiblePreset::LmStudio, "LM_STUDIO_API_KEY"),
            (OpenAiCompatiblePreset::OpenRouter, "OPENROUTER_API_KEY"),
            (OpenAiCompatiblePreset::TogetherAi, "TOGETHER_API_KEY"),
            (OpenAiCompatiblePreset::FireworksAi, "FIREWORKS_API_KEY"),
            (OpenAiCompatiblePreset::Groq, "GROQ_API_KEY"),
            (OpenAiCompatiblePreset::Custom, "CUSTOM_OPENAI_API_KEY"),
        ];

        for (preset, env_var) in presets {
            let api_key = std::env::var(env_var).ok();
            let cfg = OpenAiCompatibleConfig::preset(preset, api_key);
            reg.register(cfg.name.clone(), Arc::new(OpenAiCompatibleProvider::new(cfg)));
        }

        let anthropic_key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_default();
        reg.register(
            "anthropic",
            Arc::new(AnthropicProvider::new(AnthropicConfig::new(anthropic_key))),
        );

        let gemini_key = std::env::var("GEMINI_API_KEY").unwrap_or_default();
        reg.register(
            "gemini",
            Arc::new(GeminiProvider::new(GeminiConfig::new(gemini_key))),
        );

        reg
    }

    pub fn register(&mut self, name: impl Into<String>, provider: Arc<dyn ModelProvider>) {
        self.providers.insert(name.into(), provider);
    }

    pub fn get(&self, name: &str) -> Result<Arc<dyn ModelProvider>> {
        self.providers
            .get(name)
            .cloned()
            .ok_or_else(|| BugHunterError::NotFound(format!("provider '{name}' not registered")))
    }

    pub fn names(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    pub async fn list_capabilities(&self) -> Vec<ModelCapabilities> {
        let mut list = Vec::with_capacity(self.providers.len());
        for provider in self.providers.values() {
            if let Ok(caps) = provider.capabilities().await {
                list.push(caps);
            }
        }
        list
    }
}
