use bughunter_core::{
    BugHunterError, DebuggingResult, HealthStatus, InvestigationRecord, InvestigationStatus,
    Report, Result,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartInvestigationRequest {
    pub repository: String,
    pub issue: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    pub patch: Option<String>,
    #[serde(default)]
    pub verification_required: bool,
    pub provider_hint: Option<String>,
    pub model_hint: Option<String>,
}

impl StartInvestigationRequest {
    pub fn new(repository: impl Into<String>) -> Self {
        Self {
            repository: repository.into(),
            issue: None,
            files: Vec::new(),
            patch: None,
            verification_required: false,
            provider_hint: None,
            model_hint: None,
        }
    }

    pub fn with_issue(mut self, issue: impl Into<String>) -> Self {
        self.issue = Some(issue.into());
        self
    }

    pub fn with_verification(mut self, required: bool) -> Self {
        self.verification_required = required;
        self
    }
}

/// Official Rust SDK client for the BugHunter REST API v1.
#[derive(Clone)]
pub struct BugHunterClient {
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl BugHunterClient {
    pub fn new(base_url: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            http: reqwest::Client::new(),
        }
    }

    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => builder.bearer_auth(key),
            None => builder,
        }
    }

    async fn parse_response<T: serde::de::DeserializeOwned>(
        &self,
        resp: reqwest::Response,
    ) -> Result<T> {
        let status = resp.status();
        if status.is_success() {
            return resp
                .json::<T>()
                .await
                .map_err(|e| BugHunterError::Internal(format!("failed to decode response: {e}")));
        }
        let body = resp.text().await.unwrap_or_default();
        Err(match status.as_u16() {
            401 => BugHunterError::Authentication(body),
            403 => BugHunterError::Authorization(body),
            404 => BugHunterError::NotFound(body),
            429 => BugHunterError::RateLimited(body),
            400 | 422 => BugHunterError::Validation(body),
            _ => BugHunterError::Internal(format!("HTTP {}: {}", status.as_u16(), body)),
        })
    }

    pub async fn start_investigation(
        &self,
        req: &StartInvestigationRequest,
    ) -> Result<InvestigationRecord> {
        let url = format!("{}/api/v1/investigations", self.base_url);
        let resp = self
            .apply_auth(self.http.post(&url).json(req))
            .send()
            .await
            .map_err(|e| BugHunterError::Internal(e.to_string()))?;
        self.parse_response(resp).await
    }

    pub async fn get_status(&self, investigation_id: &str) -> Result<InvestigationRecord> {
        let url = format!("{}/api/v1/investigations/{investigation_id}", self.base_url);
        let resp = self
            .apply_auth(self.http.get(&url))
            .send()
            .await
            .map_err(|e| BugHunterError::Internal(e.to_string()))?;
        self.parse_response(resp).await
    }

    pub async fn get_report(&self, investigation_id: &str) -> Result<Report> {
        let url = format!(
            "{}/api/v1/investigations/{investigation_id}/report",
            self.base_url
        );
        let resp = self
            .apply_auth(self.http.get(&url))
            .send()
            .await
            .map_err(|e| BugHunterError::Internal(e.to_string()))?;
        self.parse_response(resp).await
    }

    pub async fn cancel_investigation(&self, investigation_id: &str) -> Result<InvestigationRecord> {
        let url = format!(
            "{}/api/v1/investigations/{investigation_id}/cancel",
            self.base_url
        );
        let resp = self
            .apply_auth(self.http.post(&url))
            .send()
            .await
            .map_err(|e| BugHunterError::Internal(e.to_string()))?;
        self.parse_response(resp).await
    }

    pub async fn wait_for_investigation(
        &self,
        investigation_id: &str,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<DebuggingResult> {
        let start = std::time::Instant::now();
        loop {
            let rec = self.get_status(investigation_id).await?;
            match rec.status {
                InvestigationStatus::Completed => {
                    return rec.result.ok_or_else(|| {
                        BugHunterError::Internal("completed investigation missing result".into())
                    });
                }
                InvestigationStatus::Failed => {
                    return Err(BugHunterError::Internal(
                        rec.error.unwrap_or_else(|| "investigation failed".into()),
                    ));
                }
                InvestigationStatus::Cancelled => {
                    return Err(BugHunterError::Cancelled(investigation_id.to_string()));
                }
                _ => {
                    if start.elapsed() >= timeout {
                        return Err(BugHunterError::Timeout(format!(
                            "timed out waiting for investigation {investigation_id}"
                        )));
                    }
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    pub async fn analyze_repository(
        &self,
        req: &StartInvestigationRequest,
    ) -> Result<DebuggingResult> {
        let url = format!("{}/api/v1/repositories/analyze", self.base_url);
        let resp = self
            .apply_auth(self.http.post(&url).json(req))
            .send()
            .await
            .map_err(|e| BugHunterError::Internal(e.to_string()))?;
        self.parse_response(resp).await
    }

    pub async fn health(&self) -> Result<HealthStatus> {
        let url = format!("{}/api/v1/health", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| BugHunterError::Internal(e.to_string()))?;
        self.parse_response(resp).await
    }
}
