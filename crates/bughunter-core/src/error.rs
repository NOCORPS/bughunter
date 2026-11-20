use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Unified error type for all BugHunter core and integration layers.
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum BugHunterError {
    #[error("authentication required or invalid credentials: {0}")]
    Authentication(String),

    #[error("permission denied: {0}")]
    Authorization(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("resource not found: {0}")]
    NotFound(String),

    #[error("rate limit exceeded: {0}")]
    RateLimited(String),

    #[error("operation timed out: {0}")]
    Timeout(String),

    #[error("provider error ({provider}): {message}")]
    Provider {
        provider: String,
        message: String,
        retryable: bool,
        status_code: Option<u16>,
    },

    #[error("sandbox policy violation: {0}")]
    SandboxViolation(String),

    #[error("human approval required: {0}")]
    ApprovalRequired(String),

    #[error("investigation cancelled: {0}")]
    Cancelled(String),

    #[error("unsupported capability: {0}")]
    Unsupported(String),

    #[error("internal error: {0}")]
    Internal(String),
}

impl BugHunterError {
    /// Returns a stable machine-readable error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Authentication(_) => "AUTHENTICATION_ERROR",
            Self::Authorization(_) => "AUTHORIZATION_ERROR",
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::NotFound(_) => "NOT_FOUND",
            Self::RateLimited(_) => "RATE_LIMITED",
            Self::Timeout(_) => "TIMEOUT",
            Self::Provider { .. } => "PROVIDER_ERROR",
            Self::SandboxViolation(_) => "SANDBOX_VIOLATION",
            Self::ApprovalRequired(_) => "APPROVAL_REQUIRED",
            Self::Cancelled(_) => "CANCELLED",
            Self::Unsupported(_) => "UNSUPPORTED_CAPABILITY",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    /// Returns whether the operation may succeed on retry.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited(_) | Self::Timeout(_) => true,
            Self::Provider { retryable, .. } => *retryable,
            _ => false,
        }
    }

    /// Suggested HTTP status code for API & HTTP transports.
    pub fn http_status_code(&self) -> u16 {
        match self {
            Self::Authentication(_) => 401,
            Self::Authorization(_) => 403,
            Self::Validation(_) => 400,
            Self::NotFound(_) => 404,
            Self::RateLimited(_) => 429,
            Self::Timeout(_) => 504,
            Self::Provider { status_code, .. } => status_code.unwrap_or(502),
            Self::SandboxViolation(_) => 403,
            Self::ApprovalRequired(_) => 409,
            Self::Cancelled(_) => 409,
            Self::Unsupported(_) => 501,
            Self::Internal(_) => 500,
        }
    }
}

pub type Result<T> = std::result::Result<T, BugHunterError>;
