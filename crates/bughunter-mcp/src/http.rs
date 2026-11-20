use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::server::McpServer;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use bughunter_core::{BugHunterCore, IntegrationSource};
use std::convert::Infallible;
use std::sync::Arc;

#[derive(Clone)]
pub struct McpHttpState {
    pub core: Arc<BugHunterCore>,
    pub server: Arc<McpServer>,
    pub require_auth: bool,
}

pub fn mcp_router(core: Arc<BugHunterCore>, require_auth: bool) -> Router {
    let server = Arc::new(McpServer::new(Arc::clone(&core)));
    let state = McpHttpState {
        core,
        server,
        require_auth,
    };
    Router::new()
        .route("/mcp", post(handle_mcp_http))
        .with_state(state)
}

async fn handle_mcp_http(
    State(state): State<McpHttpState>,
    headers: HeaderMap,
    Json(req): Json<JsonRpcRequest>,
) -> Response {
    let ctx = if state.require_auth {
        let auth_hdr = headers.get("authorization").and_then(|v| v.to_str().ok());
        match state.core.security.authenticate_bearer(auth_hdr) {
            Ok(c) => c,
            Err(e) => {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(JsonRpcResponse::err(req.id, -32001, e.to_string(), None)),
                )
                    .into_response();
            }
        }
    } else {
        bughunter_core::AuthContext::local_cli()
    };

    let wants_sse = headers
        .get("accept")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.contains("text/event-stream"))
        .unwrap_or(false);

    match state
        .server
        .handle_request(&ctx, IntegrationSource::McpHttp, req)
        .await
    {
        None => StatusCode::ACCEPTED.into_response(),
        Some(resp) if wants_sse => {
            let payload = serde_json::to_string(&resp).unwrap_or_default();
            let stream = async move {
                let ev = Event::default().event("message").data(payload);
                Ok::<Event, Infallible>(ev)
            };
            Sse::new(tokio_stream_once(stream))
                .keep_alive(KeepAlive::default())
                .into_response()
        }
        Some(resp) => (StatusCode::OK, Json(resp)).into_response(),
    }
}

fn tokio_stream_once<F>(fut: F) -> impl futures::Stream<Item = Result<Event, Infallible>>
where
    F: std::future::Future<Output = Result<Event, Infallible>>,
{
    futures::stream::once(fut)
}
