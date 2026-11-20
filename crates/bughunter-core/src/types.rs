use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Origin of a debugging request for auditing and telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationSource {
    Cli,
    McpStdio,
    McpHttp,
    A2a,
    RestApi,
    RustSdk,
    PythonSdk,
    AgentAdapter(String),
    Internal,
}

/// Supported debugging workflows in BugHunter Core.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DebuggingWorkflow {
    Investigate,
    AnalyzeRepository,
    Reproduce,
    Diagnose,
    GeneratePatch,
    Verify,
}

/// Normalized internal debugging request.
/// All external integrations (CLI, MCP, A2A, REST, SDKs) MUST normalize into this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebuggingRequest {
    pub request_id: String,
    pub source: IntegrationSource,
    pub workflow: DebuggingWorkflow,
    pub repository: String,
    pub issue: Option<String>,
    pub target_ref: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    pub patch: Option<String>,
    #[serde(default)]
    pub verification_required: bool,
    pub provider_hint: Option<String>,
    pub model_hint: Option<String>,
    #[serde(default)]
    pub parameters: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

impl DebuggingRequest {
    pub fn new(
        source: IntegrationSource,
        workflow: DebuggingWorkflow,
        repository: impl Into<String>,
    ) -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
            source,
            workflow,
            repository: repository.into(),
            issue: None,
            target_ref: None,
            files: Vec::new(),
            patch: None,
            verification_required: false,
            provider_hint: None,
            model_hint: None,
            parameters: BTreeMap::new(),
            metadata: BTreeMap::new(),
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

    pub fn with_patch(mut self, patch: impl Into<String>) -> Self {
        self.patch = Some(patch.into());
        self
    }

    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files = files;
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InvestigationStatus {
    Pending,
    Running,
    AwaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryAnalysis {
    pub repository: String,
    pub is_local_path: bool,
    pub detected_languages: Vec<String>,
    pub detected_frameworks: Vec<String>,
    pub file_count: usize,
    pub candidate_files: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReproductionResult {
    pub reproduced: bool,
    pub reproduction_steps: Vec<String>,
    pub observed_failure: Option<String>,
    pub environment: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosisResult {
    pub root_cause: String,
    pub confidence: f32,
    pub affected_components: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchResult {
    pub unified_diff: String,
    pub files_modified: Vec<String>,
    pub rationale: String,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub verified: bool,
    pub checks_run: Vec<String>,
    pub passed_checks: usize,
    pub failed_checks: usize,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub investigation_id: String,
    pub title: String,
    pub executive_summary: String,
    pub workflow: DebuggingWorkflow,
    pub status: InvestigationStatus,
    pub findings: Vec<String>,
    pub recommendations: Vec<String>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub duration_ms: u64,
    pub files_inspected: usize,
    pub source_integration: IntegrationSource,
}

/// Normalized internal debugging result returned by BugHunter Core.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebuggingResult {
    pub investigation_id: String,
    pub request_id: String,
    pub status: InvestigationStatus,
    pub workflow: DebuggingWorkflow,
    pub summary: String,
    pub repository_analysis: Option<RepositoryAnalysis>,
    pub reproduction: Option<ReproductionResult>,
    pub diagnosis: Option<DiagnosisResult>,
    pub patch: Option<PatchResult>,
    pub verification: Option<VerificationResult>,
    pub report: Report,
    pub metrics: ExecutionMetrics,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Stored investigation record for asynchronous tracking and status queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvestigationRecord {
    pub id: String,
    pub request: DebuggingRequest,
    pub status: InvestigationStatus,
    pub result: Option<DebuggingResult>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub uptime_seconds: u64,
    pub active_investigations: usize,
    pub total_investigations: usize,
}
