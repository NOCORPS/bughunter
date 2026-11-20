use crate::openapi::openapi_spec;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bughunter_a2a::a2a_router;
use bughunter_core::{
    AuthContext, BugHunterCore, BugHunterError, DebuggingRequest, DebuggingWorkflow,
    IntegrationSource,
};
use bughunter_mcp::mcp_router;
use bughunter_providers::ProviderRegistry;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiState {
    pub core: Arc<BugHunterCore>,
    pub providers: Arc<ProviderRegistry>,
    pub require_auth: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRequestBody {
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

impl WorkflowRequestBody {
    fn into_debugging_request(self, workflow: DebuggingWorkflow) -> DebuggingRequest {
        let mut req = DebuggingRequest::new(IntegrationSource::RestApi, workflow, self.repository);
        req.issue = self.issue;
        req.files = self.files;
        req.patch = self.patch;
        req.verification_required = self.verification_required;
        req.provider_hint = self.provider_hint;
        req.model_hint = self.model_hint;
        req
    }
}

pub fn api_router(
    core: Arc<BugHunterCore>,
    providers: Arc<ProviderRegistry>,
    require_auth: bool,
) -> Router {
    let state = ApiState {
        core,
        providers,
        require_auth,
    };

    Router::new()
        .route("/api/v1/investigations", post(create_investigation))
        .route("/api/v1/investigations/{id}", get(get_investigation))
        .route("/api/v1/investigations/{id}/cancel", post(cancel_investigation))
        .route("/api/v1/investigations/{id}/report", get(get_investigation_report))
        .route("/api/v1/repositories/analyze", post(analyze_repository))
        .route("/api/v1/bugs/reproduce", post(reproduce_bug))
        .route("/api/v1/patches/generate", post(generate_patch))
        .route("/api/v1/patches/verify", post(verify_patch))
        .route("/api/v1/providers", get(list_providers))
        .route("/api/v1/health", get(health_check))
        .route("/api/v1/openapi.json", get(get_openapi))
        .with_state(state)
}

/// Builds a unified server router exposing REST API v1, MCP Streamable HTTP, and A2A.
pub fn unified_server_router(
    core: Arc<BugHunterCore>,
    providers: Arc<ProviderRegistry>,
    base_url: impl Into<String>,
    require_auth: bool,
) -> Router {
    api_router(Arc::clone(&core), providers, require_auth)
        .merge(mcp_router(Arc::clone(&core), require_auth))
        .merge(a2a_router(core, base_url, require_auth))
}

fn auth_from_headers(state: &ApiState, headers: &HeaderMap) -> Result<AuthContext, BugHunterError> {
    if !state.require_auth {
        return Ok(AuthContext::local_cli());
    }
    let hdr = headers.get("authorization").and_then(|v| v.to_str().ok());
    state.core.security.authenticate_bearer(hdr)
}

fn error_response(err: BugHunterError) -> Response {
    let status =
        StatusCode::from_u16(err.http_status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(serde_json::json!({
            "error": {
                "code": err.code(),
                "message": err.to_string(),
                "retryable": err.is_retryable(),
            }
        })),
    )
        .into_response()
}

async fn create_investigation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<WorkflowRequestBody>,
) -> Response {
    let ctx = match auth_from_headers(&state, &headers) {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    let req = body.into_debugging_request(DebuggingWorkflow::Investigate);
    match state.core.start_investigation(&ctx, req).await {
        Ok(rec) => (StatusCode::ACCEPTED, Json(rec)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn get_investigation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let ctx = match auth_from_headers(&state, &headers) {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    match state.core.get_investigation(&ctx, &id) {
        Ok(rec) => (StatusCode::OK, Json(rec)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn cancel_investigation(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let ctx = match auth_from_headers(&state, &headers) {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    match state.core.cancel_investigation(&ctx, &id) {
        Ok(rec) => (StatusCode::OK, Json(rec)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn get_investigation_report(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let ctx = match auth_from_headers(&state, &headers) {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    match state.core.get_report(&ctx, &id) {
        Ok(rep) => (StatusCode::OK, Json(rep)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn run_sync_workflow(
    state: ApiState,
    headers: HeaderMap,
    workflow: DebuggingWorkflow,
    body: WorkflowRequestBody,
) -> Response {
    let ctx = match auth_from_headers(&state, &headers) {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    let req = body.into_debugging_request(workflow);
    match state.core.execute(&ctx, req).await {
        Ok(res) => (StatusCode::OK, Json(res)).into_response(),
        Err(e) => error_response(e),
    }
}

async fn analyze_repository(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<WorkflowRequestBody>,
) -> Response {
    run_sync_workflow(state, headers, DebuggingWorkflow::AnalyzeRepository, body).await
}

async fn reproduce_bug(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<WorkflowRequestBody>,
) -> Response {
    run_sync_workflow(state, headers, DebuggingWorkflow::Reproduce, body).await
}

async fn generate_patch(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<WorkflowRequestBody>,
) -> Response {
    run_sync_workflow(state, headers, DebuggingWorkflow::GeneratePatch, body).await
}

async fn verify_patch(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(body): Json<WorkflowRequestBody>,
) -> Response {
    run_sync_workflow(state, headers, DebuggingWorkflow::Verify, body).await
}

async fn list_providers(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    if let Err(e) = auth_from_headers(&state, &headers) {
        return error_response(e);
    }
    let caps = state.providers.list_capabilities().await;
    (StatusCode::OK, Json(serde_json::json!({ "providers": caps }))).into_response()
}

async fn health_check(State(state): State<ApiState>) -> Response {
    (StatusCode::OK, Json(state.core.health())).into_response()
}

async fn get_openapi() -> Response {
    (StatusCode::OK, Json(openapi_spec())).into_response()
}
