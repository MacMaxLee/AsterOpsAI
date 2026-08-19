use std::path::PathBuf;
use std::sync::Arc;

use ai_ops_core::repository::{self, RepositoryConfig};
use clap::{Parser, Subcommand};
use service::{api, config, self_metrics, state::AppState, telemetry};

#[derive(Parser)]
#[command(name = "ai-ops-core", about = "AsterOpsAI core service")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the core service (default when no subcommand is given).
    Serve,
    Audit {
        #[command(subcommand)]
        action: AuditCommand,
    },
}

#[derive(Subcommand)]
enum AuditCommand {
    /// Walk the audit hash chain and report the first broken link, if any.
    Verify {
        /// Defaults to the same path the service itself would use.
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => serve().await,
        Command::Audit {
            action: AuditCommand::Verify { db },
        } => {
            std::process::exit(audit_verify(db));
        }
    }
}

async fn serve() -> anyhow::Result<()> {
    let platform: Arc<dyn platform::PlatformAdapter> =
        Arc::from(platform::current_platform_adapter());
    let self_metrics = self_metrics::spawn(platform.clone());

    let repository = match config::resolve_default_db_path() {
        Ok(db_path) => match repository::init(&RepositoryConfig { db_path }) {
            Ok(handle) => Some(handle),
            Err(err) => {
                tracing::error!(error = %err, "repository layer did not start; continuing without persistence");
                None
            }
        },
        Err(err) => {
            tracing::error!(error = %err, "could not resolve a database path; continuing without persistence");
            None
        }
    };

    let host_telemetry = telemetry::sampler::spawn(platform.clone(), repository.clone());
    let state = AppState::new(platform, self_metrics, host_telemetry, repository);
    let app = api::router(state);

    #[cfg(unix)]
    {
        let socket_path = service::transport::unix::resolve_socket_path()?;
        service::transport::unix::serve(app, &socket_path).await?;
    }

    #[cfg(windows)]
    {
        service::transport::windows::serve(app).await?;
    }

    Ok(())
}

/// Returns the process exit code directly (0 verified, 1 broken link, 2
/// usage/IO error) rather than an `anyhow::Result`, so the three cases stay
/// distinct instead of collapsing to the same code the way returning `Err`
/// from `main` always would.
fn audit_verify(db: Option<PathBuf>) -> i32 {
    let db_path = match db {
        Some(path) => path,
        None => match config::resolve_default_db_path() {
            Ok(path) => path,
            Err(err) => {
                eprintln!("error: {err}");
                return 2;
            }
        },
    };

    let conn = match rusqlite::Connection::open(&db_path) {
        Ok(conn) => conn,
        Err(err) => {
            eprintln!("error opening {}: {err}", db_path.display());
            return 2;
        }
    };

    let verification = match repository::audit::verify_chain(&conn) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("error verifying chain: {err}");
            return 2;
        }
    };

    match verification.first_broken_id {
        None => {
            println!(
                "OK: audit chain verified, {} rows, no broken links",
                verification.total_rows
            );
            0
        }
        Some(id) => {
            println!(
                "BROKEN: audit chain has a broken link at id {id} ({} rows total)",
                verification.total_rows
            );
            1
        }
    }
}
