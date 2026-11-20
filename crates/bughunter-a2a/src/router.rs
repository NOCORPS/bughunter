use crate::types::{A2aTaskRequest, A2aTaskResponse, AgentCard, AgentCardCapabilities, AgentCardSkill};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use bughunter_core::{
    AuthContext, BugHunterCore, BugHunterError, DebuggingRequest, DebuggingWorkflow,
    IntegrationSource,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct A2aState {
    pub core: Arc<BugHunterCore>,
    pub base_url: String,
    pub require_auth: bool,
}

pub fn a2a_router(
    core: Arc<BugHunterCore>,
    base_url: impl Into<String>,
    require_auth: bool,
) -> Router {
    let state = A2aState {
        core,
        base_url: base_url.into(),
        require_auth,
    };
    Router::new()
        .route("/.well-known/agent.json", get(get_agent_card))
        .route("/a2a/tasks/send", post(send_task))
        .route("/a2a/tasks/{id}", get(get_task))
        .route("/a2a/tasks/{id}/cancel", post(cancel_task))
        .with_state(state)
}

async fn get_agent_card(State(state): State<A2aState>) -> Json<AgentCard> {
    Json(AgentCard {
        name: "BugHunter".to_string(),
        description: "Provider-independent, extensible autonomous debugging agent".to_string(),
        url: state.base_url,
        version: env!("CARGO_PKG_VERSION").to_string(),
        capabilities: AgentCardCapabilities {
            streaming: false,
            push_notifications: false,
            state_transition_history: true,
            async_tasks: true,
            cancellation: true,
        },
        authentication: vec!["Bearer".to_string()],
        skills: vec![
            AgentCardSkill {
                id: "debugging".to_string(),
                name: "End-to-End Debugging".to_string(),
                description: "Investigate, reproduce, diagnose, patch, and verify software bugs.".to_string(),
                tags: vec!["debugging".into(), "investigation".into(), "patching".into()],
            },
            AgentCardSkill {
                id: "repository_analysis".to_string(),
                name: "Repository Analysis".to_string(),
                description: "Inspect repository structure, languages, and frameworks.".to_string(),
                tags: vec!["analysis".into(), "repository".into()],
            },
            AgentCardSkill {
                id: "verification".to_string(),
                name: "Patch Verification".to_string(),
                description: "Verify candidate patches against tests and security policies.".to_string(),
                tags: vec!["verification".into(), "testing".into()],
            },
        ],
    })
}

fn auth_from_headers(state: &A2aState, headers: &HeaderMap) -> Result<AuthContext, BugHunterError> {
    if !state.require_auth {
        return Ok(AuthContext::local_cli());
    }
    let hdr = headers.get("authorization").and_then(|v| v.to_str().ok());
    state.core.security.authenticate_bearer(hdr)
}

fn error_response(err: BugHunterError) -> Response {
    let status = StatusCode::from_u16(err.http_status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        Json(serde_json::json!({
            "error": err.code(),
            "message": err.to_string(),
        })),
    )
        .into_response()
}

async fn send_task(
    State(state): State<A2aState>,
    headers: HeaderMap,
    Json(payload): Json<A2aTaskRequest>,
) -> Response {
    let ctx = match auth_from_headers(&state, &headers) {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };

    let workflow = match payload.capability.to_lowercase().as_str() {
        "debugging" | "investigate" => DebuggingWorkflow::Investigate,
        "analyze" | "repository_analysis" => DebuggingWorkflow::AnalyzeRepository,
        "reproduce" => DebuggingWorkflow::Reproduce,
        "diagnose" => DebuggingWorkflow::Diagnose,
        "patch" | "generate_patch" => DebuggingWorkflow::GeneratePatch,
        "verify" | "verification" => DebuggingWorkflow::Verify,
        other => {
            return error_response(BugHunterError::Validation(format!(
                "unsupported A2A capability: {other}"
            )));
        }
    };

    let mut req = DebuggingRequest::new(IntegrationSource::A2a, workflow, payload.repository);
    req.issue = payload.issue;
    req.verification_required = payload.verification_required;
    req.files = payload.files;
    req.patch = payload.patch;

    match state.core.start_investigation(&ctx, req).await {
        Ok(rec) => (
            StatusCode::ACCEPTED,
            Json(A2aTaskResponse {
                task_id: rec.id,
                status: rec.status,
                result: rec.result,
                error: rec.error,
            }),
        )
            .into_response(),
        Err(e) => error_response(e),
    }
}

async fn get_task(
    State(state): State<A2aState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let ctx = match auth_from_headers(&state, &headers) {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    match state.core.get_investigation(&ctx, &id) {
        Ok(rec) => (
            StatusCode::OK,
            Json(A2aTaskResponse {
                task_id: rec.id,
                status: rec.status,
                result: rec.result,
                error: rec.error,
            }),
        )
            .into_response(),
        Err(e) => error_response(e),
    }
}

async fn cancel_task(
    State(state): State<A2aState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let ctx = match auth_from_headers(&state, &headers) {
        Ok(c) => c,
        Err(e) => return error_response(e),
    };
    match state.core.cancel_investigation(&ctx, &id) {
        Ok(rec) => (
            StatusCode::OK,
            Json(A2aTaskResponse {
                task_id: rec.id,
                status: rec.status,
                result: rec.result,
                error: rec.error,
            }),
        )
            .into_response(),
        Err(e) => error_response(e),
    }
}
