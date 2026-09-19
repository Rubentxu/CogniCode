use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use cognicode_explorer::api;

#[derive(Debug, Parser)]
#[command(
    name = "explorer-api",
    version,
    about = "CogniCode Explorer API — moldable code exploration HTTP service.\n\n\
             LadybugDB is the sole persistence backend."
)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    #[arg(long, default_value = "127.0.0.1:8010")]
    listen: SocketAddr,

    /// Path to the LadybugDB database file.
    /// Defaults to `./cognicode.lbug` relative to cwd.
    #[arg(long)]
    db: Option<PathBuf>,

    /// Wire the Control Plane architecture read endpoint with an empty
    /// `ArchitectureRegistry` (no admitted constraints). When this flag is
    /// absent, the endpoint returns `status:incomplete` with
    /// `reason:control_query_service_not_wired` (CP1.0 fail-closed default).
    ///
    /// The registry starts empty by design — there is no fabricated
    /// architecture state. A future change will populate it from a
    /// configuration source or via an admission API; until then every
    /// response is honest about the empty admission set.
    #[arg(long)]
    with_architecture: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let runtime = if let Some(db_path) = args.db {
        // Use explicit db path via bootstrap_ladybug.

        cognicode_runtime::bootstrap_ladybug(args.cwd.clone(), db_path)?
    } else {
        // Use default `./cognicode.lbug` path.

        cognicode_runtime::bootstrap_ladybug_default(args.cwd.clone())?
    };
    let state = runtime.into_api_state();

    // CP1.0 WU5 — wire the Control Plane architecture read endpoint
    // when explicitly opted in. The registry starts empty by design; see
    // `--with-architecture` flag doc above. The source_root defaults to
    // `--cwd` so the architecture is evaluated against the workspace the
    // operator opened.
    let state = if args.with_architecture {
        use cognicode_core::application::architecture::{
            ArchitectureRegistry, ControlQueryService,
        };
        let registry = ArchitectureRegistry::new();
        tracing::info!(
            "Control Plane architecture endpoint enabled (empty ArchitectureRegistry; \
             all queries will return status:incomplete until an admission source is wired; \
             source_root={})",
            args.cwd.display()
        );
        state.with_control_query(
            Some(Arc::new(ControlQueryService::new(registry))),
            args.cwd.clone(),
        )
    } else {
        state
    };

    tracing::info!(listen = %args.listen, "starting cognicode explorer API");
    api::serve(state, args.listen).await
}
