use bughunter_agents::AgentRegistry;
use bughunter_api::unified_server_router;
use bughunter_core::{
    AuthContext, BugHunterCore, DebuggingRequest, DebuggingWorkflow, IntegrationSource,
};
use bughunter_mcp::{serve_stdio, McpServer};
use bughunter_providers::ProviderRegistry;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "bughunter",
    version,
    about = "BugHunter Universal Debugging Infrastructure CLI"
)]
struct Cli {
    /// Emit machine-readable JSON output
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Args)]
struct TargetArgs {
    /// Local repository path or owner/repository identifier
    #[arg(short, long, default_value = ".")]
    repository: String,

    /// Bug description or failing test context
    #[arg(short, long)]
    issue: Option<String>,

    /// Candidate files to inspect
    #[arg(short, long)]
    files: Vec<String>,

    /// Require verification step
    #[arg(long)]
    verify: bool,

    /// Preferred model provider
    #[arg(long)]
    provider: Option<String>,

    /// Preferred model name
    #[arg(long)]
    model: Option<String>,
}

#[derive(Clone, ValueEnum)]
enum McpTransport {
    Stdio,
    Http,
}

#[derive(Subcommand)]
enum Commands {
    /// Run a full end-to-end debugging investigation
    Investigate(TargetArgs),
    /// Analyze repository structure and candidate files
    Analyze(TargetArgs),
    /// Reproduce a reported bug
    Reproduce(TargetArgs),
    /// Diagnose root cause of a defect
    Diagnose(TargetArgs),
    /// Generate a candidate patch for a defect
    Fix(TargetArgs),
    /// Verify a candidate patch
    Verify {
        #[arg(short, long, default_value = ".")]
        repository: String,
        #[arg(short, long)]
        patch: Option<String>,
        #[arg(short, long)]
        issue: Option<String>,
    },
    /// Retrieve or generate a debugging report
    Report {
        /// Investigation ID (if omitted, runs a fresh analysis report on --repository)
        #[arg(long)]
        id: Option<String>,
        #[arg(short, long, default_value = ".")]
        repository: String,
    },
    /// Start the BugHunter MCP server
    Mcp {
        #[arg(long, value_enum, default_value_t = McpTransport::Stdio)]
        transport: McpTransport,
        #[arg(long, default_value = "127.0.0.1:8081")]
        addr: String,
    },
    /// Start the unified BugHunter HTTP server (REST API v1 + MCP HTTP + A2A)
    Serve {
        #[arg(long, default_value = "127.0.0.1:8080")]
        addr: String,
        /// Optional bearer token for authenticated remote access
        #[arg(long, env = "BUGHUNTER_API_TOKEN")]
        token: Option<String>,
    },
    /// List supported model providers and their capabilities
    Providers,
    /// List supported agent integrations and protocols
    Integrations,
}

fn build_request(workflow: DebuggingWorkflow, args: TargetArgs) -> DebuggingRequest {
    let mut req = DebuggingRequest::new(IntegrationSource::Cli, workflow, args.repository);
    req.issue = args.issue;
    req.files = args.files;
    req.verification_required = args.verify;
    req.provider_hint = args.provider;
    req.model_hint = args.model;
    req
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let cli = Cli::parse();
    let core = Arc::new(BugHunterCore::default());
    let ctx = AuthContext::local_cli();

    match cli.command {
        Commands::Investigate(args) => {
            let res = core
                .execute(&ctx, build_request(DebuggingWorkflow::Investigate, args))
                .await?;
            print_result(&res, cli.json);
        }
        Commands::Analyze(args) => {
            let res = core
                .execute(
                    &ctx,
                    build_request(DebuggingWorkflow::AnalyzeRepository, args),
                )
                .await?;
            print_result(&res, cli.json);
        }
        Commands::Reproduce(args) => {
            let res = core
                .execute(&ctx, build_request(DebuggingWorkflow::Reproduce, args))
                .await?;
            print_result(&res, cli.json);
        }
        Commands::Diagnose(args) => {
            let res = core
                .execute(&ctx, build_request(DebuggingWorkflow::Diagnose, args))
                .await?;
            print_result(&res, cli.json);
        }
        Commands::Fix(args) => {
            let res = core
                .execute(&ctx, build_request(DebuggingWorkflow::GeneratePatch, args))
                .await?;
            print_result(&res, cli.json);
        }
        Commands::Verify {
            repository,
            patch,
            issue,
        } => {
            let mut req = DebuggingRequest::new(
                IntegrationSource::Cli,
                DebuggingWorkflow::Verify,
                repository,
            );
            req.patch = patch;
            req.issue = issue;
            let res = core.execute(&ctx, req).await?;
            print_result(&res, cli.json);
        }
        Commands::Report { id, repository } => {
            if let Some(inv_id) = id {
                let rep = core.get_report(&ctx, &inv_id)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&rep)?);
                } else {
                    println!("{}: {}", rep.title, rep.executive_summary);
                }
            } else {
                let req = DebuggingRequest::new(
                    IntegrationSource::Cli,
                    DebuggingWorkflow::AnalyzeRepository,
                    repository,
                );
                let res = core.execute(&ctx, req).await?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&res.report)?);
                } else {
                    println!("{}: {}", res.report.title, res.report.executive_summary);
                }
            }
        }
        Commands::Mcp { transport, addr } => match transport {
            McpTransport::Stdio => {
                serve_stdio(McpServer::new(core), ctx).await?;
            }
            McpTransport::Http => {
                let app = bughunter_mcp::mcp_router(core, false);
                let listener = tokio::net::TcpListener::bind(&addr).await?;
                eprintln!("BugHunter MCP Streamable HTTP listening on http://{addr}/mcp");
                axum::serve(listener, app).await?;
            }
        },
        Commands::Serve { addr, token } => {
            let require_auth = token.is_some();
            if let Some(tok) = token {
                core.security.register_token(tok, AuthContext::local_cli());
            }
            let providers = Arc::new(ProviderRegistry::with_builtin_presets());
            let base_url = format!("http://{addr}");
            let app = unified_server_router(core, providers, base_url, require_auth);
            let listener = tokio::net::TcpListener::bind(&addr).await?;
            eprintln!("BugHunter unified server listening on http://{addr}");
            axum::serve(listener, app).await?;
        }
        Commands::Providers => {
            let providers = ProviderRegistry::with_builtin_presets();
            let caps = providers.list_capabilities().await;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&caps)?);
            } else {
                for c in caps {
                    println!(
                        "{:<16} default={:<32} streaming={} tools={} structured={}",
                        c.provider,
                        c.default_model,
                        c.supports_streaming,
                        c.supports_tool_calling,
                        c.supports_structured_output
                    );
                }
            }
        }
        Commands::Integrations => {
            let registry = AgentRegistry::with_builtin_adapters(core, ctx);
            let caps = registry.list_capabilities().await;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&caps)?);
            } else {
                for c in caps {
                    println!(
                        "{:<20} protocol={:?} async={} cancel={}",
                        c.name, c.protocol, c.supports_async_tasks, c.supports_cancellation
                    );
                }
            }
        }
    }

    Ok(())
}

fn print_result(res: &bughunter_core::DebuggingResult, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(res).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        println!("[{:?}] {}", res.status, res.summary);
        println!("Investigation ID: {}", res.investigation_id);
        if let Some(diag) = &res.diagnosis {
            println!("Root cause: {}", diag.root_cause);
        }
        if let Some(ver) = &res.verification {
            println!(
                "Verification: verified={} ({}/{})",
                ver.verified,
                ver.passed_checks,
                ver.passed_checks + ver.failed_checks
            );
        }
    }
}
