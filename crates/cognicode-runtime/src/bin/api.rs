use std::net::SocketAddr;
use std::path::PathBuf;

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
    tracing::info!(listen = %args.listen, "starting cognicode explorer API");
    api::serve(state, args.listen).await
}
