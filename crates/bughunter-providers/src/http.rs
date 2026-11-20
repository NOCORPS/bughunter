use crate::types::{ModelCapabilities, ModelRequest, ProviderRuntimeConfig};
use bughunter_core::{BugHunterError, Result};
use reqwest::Response;
use std::time::Duration;

/// Shared retry + error normalization helper used across all HTTP model providers.
pub async fn send_with_retries<F, Fut>(
    provider_name: &str,
    cfg: &ProviderRuntimeConfig,
    mut build_and_send: F,
) -> Result<Response>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<Response, reqwest::Error>>,
{
    let mut attempt = 0u32;
    let mut backoff_ms = cfg.initial_backoff_ms;

    loop {
        let resp_res = tokio::time::timeout(
            Duration::from_millis(cfg.timeout_ms),
            build_and_send(),
        )
        .await;

        let resp = match resp_res {
            Err(_) => {
                if attempt < cfg.max_retries {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= 2;
                    continue;
                }
                return Err(BugHunterError::Timeout(format!(
                    "provider '{provider_name}' request timed out after {}ms",
                    cfg.timeout_ms
                )));
            }
            Ok(Err(err)) => {
                if attempt < cfg.max_retries && (err.is_timeout() || err.is_connect()) {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= 2;
                    continue;
                }
                return Err(BugHunterError::Provider {
                    provider: provider_name.to_string(),
                    message: err.to_string(),
                    retryable: err.is_timeout() || err.is_connect(),
                    status_code: err.status().map(|s| s.as_u16()),
                });
            }
            Ok(Ok(r)) => r,
        };

        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }

        let retry_after_ms = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .map(|secs| secs * 1000);

        let body_text = resp.text().await.unwrap_or_default();
        let code = status.as_u16();
        let retryable = code == 429 || (500..600).contains(&code);

        if retryable && attempt < cfg.max_retries {
            attempt += 1;
            let wait = retry_after_ms.unwrap_or(backoff_ms).min(10_000);
            tokio::time::sleep(Duration::from_millis(wait)).await;
            backoff_ms *= 2;
            continue;
        }

        return Err(normalize_http_error(provider_name, code, body_text));
    }
}

pub fn normalize_http_error(provider: &str, status_code: u16, body: String) -> BugHunterError {
    match status_code {
        401 | 403 => BugHunterError::Authentication(format!(
            "provider '{provider}' rejected credentials (HTTP {status_code}): {body}"
        )),
        429 => BugHunterError::RateLimited(format!(
            "provider '{provider}' rate limit exceeded (HTTP 429): {body}"
        )),
        400 | 422 => BugHunterError::Validation(format!(
            "provider '{provider}' rejected request (HTTP {status_code}): {body}"
        )),
        _ => BugHunterError::Provider {
            provider: provider.to_string(),
            message: format!("HTTP {status_code}: {body}"),
            retryable: (500..600).contains(&status_code),
            status_code: Some(status_code),
        },
    }
}

/// Validates context window & applies graceful fallback instructions when a provider
/// lacks native structured-output or tool-calling support.
pub fn prepare_request_with_fallbacks(
    mut req: ModelRequest,
    caps: &ModelCapabilities,
) -> Result<ModelRequest> {
    let est_tokens = req.estimated_input_tokens();
    if est_tokens > caps.max_context_tokens {
        return Err(BugHunterError::Validation(format!(
            "estimated input tokens ({est_tokens}) exceeds provider '{}' context window ({})",
            caps.provider, caps.max_context_tokens
        )));
    }

    if !caps.supports_structured_output {
        if let Some(schema) = req.response_schema.take() {
            let instruction = format!(
                "\nRespond ONLY with valid JSON matching schema '{}': {}",
                schema.name, schema.schema
            );
            req.messages
                .push(crate::types::ChatMessage::system(instruction));
        }
    }

    if !caps.supports_tool_calling && !req.tools.is_empty() {
        let tool_names: Vec<String> = req.tools.iter().map(|t| t.name.clone()).collect();
        req.tools.clear();
        req.messages.push(crate::types::ChatMessage::system(format!(
            "Note: Native tool calling is unavailable; available conceptual tools: {}",
            tool_names.join(", ")
        )));
    }

    Ok(req)
}
