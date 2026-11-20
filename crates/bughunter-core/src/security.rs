use crate::error::{BugHunterError, Result};
use crate::types::{DebuggingRequest, DebuggingWorkflow, IntegrationSource};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Fine-grained permissions enforced across all BugHunter integrations.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ReadRepository,
    Investigate,
    Reproduce,
    Diagnose,
    GeneratePatch,
    VerifyPatch,
    ExecuteTools,
    ManageProviders,
    ManagePlugins,
    Admin,
}

impl Permission {
    pub fn for_workflow(workflow: DebuggingWorkflow) -> Self {
        match workflow {
            DebuggingWorkflow::Investigate => Self::Investigate,
            DebuggingWorkflow::AnalyzeRepository => Self::ReadRepository,
            DebuggingWorkflow::Reproduce => Self::Reproduce,
            DebuggingWorkflow::Diagnose => Self::Diagnose,
            DebuggingWorkflow::GeneratePatch => Self::GeneratePatch,
            DebuggingWorkflow::Verify => Self::VerifyPatch,
        }
    }

    pub fn all() -> HashSet<Self> {
        [
            Self::ReadRepository,
            Self::Investigate,
            Self::Reproduce,
            Self::Diagnose,
            Self::GeneratePatch,
            Self::VerifyPatch,
            Self::ExecuteTools,
            Self::ManageProviders,
            Self::ManagePlugins,
            Self::Admin,
        ]
        .into_iter()
        .collect()
    }

    pub fn standard_agent() -> HashSet<Self> {
        [
            Self::ReadRepository,
            Self::Investigate,
            Self::Reproduce,
            Self::Diagnose,
            Self::GeneratePatch,
            Self::VerifyPatch,
        ]
        .into_iter()
        .collect()
    }
}

/// Authenticated caller context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub principal: String,
    pub auth_method: String,
    pub scopes: HashSet<Permission>,
    pub credential_id: Option<String>,
    pub human_approved: bool,
}

impl AuthContext {
    pub fn local_cli() -> Self {
        Self {
            principal: "local-cli-user".to_string(),
            auth_method: "local_session".to_string(),
            scopes: Permission::all(),
            credential_id: None,
            human_approved: true,
        }
    }

    pub fn scoped_agent(
        principal: impl Into<String>,
        scopes: HashSet<Permission>,
        human_approved: bool,
    ) -> Self {
        Self {
            principal: principal.into(),
            auth_method: "scoped_token".to_string(),
            scopes,
            credential_id: None,
            human_approved,
        }
    }

    pub fn has_permission(&self, perm: Permission) -> bool {
        self.scopes.contains(&Permission::Admin) || self.scopes.contains(&perm)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_execution_seconds: u64,
    pub max_files_inspected: usize,
    pub max_output_bytes: usize,
    pub max_concurrent_investigations: usize,
    pub rate_limit_per_minute: usize,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_execution_seconds: 120,
            max_files_inspected: 500,
            max_output_bytes: 1_048_576,
            max_concurrent_investigations: 16,
            rate_limit_per_minute: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxPolicy {
    pub allow_network_for_repro: bool,
    pub allow_filesystem_write: bool,
    pub blocked_path_prefixes: Vec<String>,
    pub require_human_approval_for_patch: bool,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            allow_network_for_repro: false,
            allow_filesystem_write: false,
            blocked_path_prefixes: vec![
                "/etc".to_string(),
                "/root".to_string(),
                "/proc".to_string(),
                "/sys".to_string(),
            ],
            require_human_approval_for_patch: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub principal: String,
    pub source: IntegrationSource,
    pub action: String,
    pub target: String,
    pub allowed: bool,
    pub reason: Option<String>,
}

pub trait AuditLogger: Send + Sync {
    fn log(&self, event: AuditEvent);
    fn events(&self) -> Vec<AuditEvent>;
}

#[derive(Debug, Default)]
pub struct InMemoryAuditLogger {
    events: Mutex<Vec<AuditEvent>>,
}

impl AuditLogger for InMemoryAuditLogger {
    fn log(&self, event: AuditEvent) {
        if let Ok(mut guard) = self.events.lock() {
            guard.push(event);
        }
    }

    fn events(&self) -> Vec<AuditEvent> {
        self.events
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    }
}

/// Redacts common secret patterns from logs, errors, and reports.
pub struct SecretRedactor;

impl SecretRedactor {
    pub fn redact(input: &str) -> String {
        static PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
        let patterns = PATTERNS.get_or_init(|| {
            vec![
                (
                    Regex::new(r"(?i)(bearer\s+)[a-zA-Z0-9._\-]{8,}").unwrap(),
                    "${1}[REDACTED]",
                ),
                (
                    Regex::new(r"\bsk-[a-zA-Z0-9_\-]{12,}\b").unwrap(),
                    "[REDACTED_API_KEY]",
                ),
                (
                    Regex::new(r"\bghp_[a-zA-Z0-9]{16,}\b").unwrap(),
                    "[REDACTED_GH_TOKEN]",
                ),
                (
                    Regex::new(r"(?i)(api[_-]?key|secret|password|token)\s*[:=]\s*[^\s,;]+")
                        .unwrap(),
                    "${1}=[REDACTED]",
                ),
            ]
        });

        let mut output = input.to_string();
        for (re, replacement) in patterns {
            output = re.replace_all(&output, *replacement).to_string();
        }
        output
    }
}

/// Sliding-window rate limiter keyed by principal.
#[derive(Debug, Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<String, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn check_and_record(&self, principal: &str, max_per_minute: usize) -> Result<()> {
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(60);
        let mut guard = self
            .windows
            .lock()
            .map_err(|_| BugHunterError::Internal("rate limiter lock poisoned".into()))?;
        let timestamps = guard.entry(principal.to_string()).or_default();
        timestamps.retain(|&ts| ts > cutoff);
        if timestamps.len() >= max_per_minute {
            return Err(BugHunterError::RateLimited(format!(
                "principal '{principal}' exceeded {max_per_minute} requests/min"
            )));
        }
        timestamps.push(now);
        Ok(())
    }
}

/// Centralized security policy enforced identically across all integrations.
#[derive(Clone)]
pub struct SecurityPolicy {
    pub limits: ResourceLimits,
    pub sandbox: SandboxPolicy,
    pub valid_tokens: Arc<Mutex<HashMap<String, AuthContext>>>,
    pub rate_limiter: Arc<RateLimiter>,
    pub audit_logger: Arc<dyn AuditLogger>,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            limits: ResourceLimits::default(),
            sandbox: SandboxPolicy::default(),
            valid_tokens: Arc::new(Mutex::new(HashMap::new())),
            rate_limiter: Arc::new(RateLimiter::default()),
            audit_logger: Arc::new(InMemoryAuditLogger::default()),
        }
    }
}

impl SecurityPolicy {
    pub fn register_token(&self, token: impl Into<String>, ctx: AuthContext) {
        if let Ok(mut guard) = self.valid_tokens.lock() {
            guard.insert(token.into(), ctx);
        }
    }

    pub fn authenticate_bearer(&self, header_value: Option<&str>) -> Result<AuthContext> {
        let raw = header_value.ok_or_else(|| {
            BugHunterError::Authentication("missing Authorization header".to_string())
        })?;
        let token = raw
            .strip_prefix("Bearer ")
            .or_else(|| raw.strip_prefix("bearer "))
            .unwrap_or(raw)
            .trim();
        if token.is_empty() {
            return Err(BugHunterError::Authentication(
                "empty bearer token".to_string(),
            ));
        }
        let guard = self
            .valid_tokens
            .lock()
            .map_err(|_| BugHunterError::Internal("token store lock poisoned".into()))?;
        guard.get(token).cloned().ok_or_else(|| {
            BugHunterError::Authentication("invalid or expired credentials".to_string())
        })
    }

    pub fn authorize_request(&self, ctx: &AuthContext, req: &DebuggingRequest) -> Result<()> {
        // 1. Rate limit check
        if let Err(err) = self
            .rate_limiter
            .check_and_record(&ctx.principal, self.limits.rate_limit_per_minute)
        {
            self.record_audit(ctx, req, false, Some(err.to_string()));
            return Err(err);
        }

        // 2. Permission check
        let required = Permission::for_workflow(req.workflow);
        if !ctx.has_permission(required) {
            let msg = format!(
                "principal '{}' lacks permission {:?}",
                ctx.principal, required
            );
            self.record_audit(ctx, req, false, Some(msg.clone()));
            return Err(BugHunterError::Authorization(msg));
        }

        // 3. Request validation
        if req.repository.trim().is_empty() {
            let msg = "repository must not be empty".to_string();
            self.record_audit(ctx, req, false, Some(msg.clone()));
            return Err(BugHunterError::Validation(msg));
        }
        if req.repository.contains("..") {
            let msg = "repository path traversal ('..') is forbidden".to_string();
            self.record_audit(ctx, req, false, Some(msg.clone()));
            return Err(BugHunterError::SandboxViolation(msg));
        }

        // 4. Sandbox path check for local filesystem paths
        let repo_path = Path::new(&req.repository);
        if repo_path.is_absolute() {
            for blocked in &self.sandbox.blocked_path_prefixes {
                if req.repository.starts_with(blocked) {
                    let msg = format!("access to blocked path prefix '{blocked}' denied");
                    self.record_audit(ctx, req, false, Some(msg.clone()));
                    return Err(BugHunterError::SandboxViolation(msg));
                }
            }
        }

        // 5. Human approval gate when configured for patch generation
        if req.workflow == DebuggingWorkflow::GeneratePatch
            && self.sandbox.require_human_approval_for_patch
            && !ctx.human_approved
        {
            let msg = "human approval is required for patch generation".to_string();
            self.record_audit(ctx, req, false, Some(msg.clone()));
            return Err(BugHunterError::ApprovalRequired(msg));
        }

        self.record_audit(ctx, req, true, None);
        Ok(())
    }

    pub fn authorize_permission(
        &self,
        ctx: &AuthContext,
        source: IntegrationSource,
        perm: Permission,
        target: &str,
    ) -> Result<()> {
        let allowed = ctx.has_permission(perm);
        self.audit_logger.log(AuditEvent {
            timestamp: Utc::now(),
            principal: ctx.principal.clone(),
            source,
            action: format!("{perm:?}"),
            target: SecretRedactor::redact(target),
            allowed,
            reason: if allowed {
                None
            } else {
                Some(format!("missing permission {perm:?}"))
            },
        });
        if !allowed {
            return Err(BugHunterError::Authorization(format!(
                "principal '{}' lacks permission {:?}",
                ctx.principal, perm
            )));
        }
        Ok(())
    }

    fn record_audit(
        &self,
        ctx: &AuthContext,
        req: &DebuggingRequest,
        allowed: bool,
        reason: Option<String>,
    ) {
        self.audit_logger.log(AuditEvent {
            timestamp: Utc::now(),
            principal: ctx.principal.clone(),
            source: req.source.clone(),
            action: format!("{:?}", req.workflow),
            target: SecretRedactor::redact(&req.repository),
            allowed,
            reason: reason.map(|r| SecretRedactor::redact(&r)),
        });
    }
}
