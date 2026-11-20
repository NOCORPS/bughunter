pub mod engine;
pub mod error;
pub mod security;
pub mod types;

pub use engine::BugHunterCore;
pub use error::{BugHunterError, Result};
pub use security::{
    AuditEvent, AuditLogger, AuthContext, InMemoryAuditLogger, Permission, RateLimiter,
    ResourceLimits, SandboxPolicy, SecretRedactor, SecurityPolicy,
};
pub use types::{
    DebuggingRequest, DebuggingResult, DebuggingWorkflow, DiagnosisResult, ExecutionMetrics,
    HealthStatus, IntegrationSource, InvestigationRecord, InvestigationStatus, PatchResult,
    Report, RepositoryAnalysis, ReproductionResult, VerificationResult,
};
