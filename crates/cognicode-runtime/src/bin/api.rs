use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use cognicode_explorer::api;

#[derive(Debug, Parser)]
#[command(
    name = "explorer-api",
    version,
    about = "CogniCode Explorer API — moldable code exploration HTTP service.\n\n\
             Precedence: --postgres <URL> > DATABASE_URL. \
             No flag and no env is fatal. See docs/postgres-default-config for details."
)]
struct Args {
    #[arg(short, long, default_value = ".")]
    cwd: PathBuf,

    #[arg(long, default_value = "127.0.0.1:8010")]
    listen: SocketAddr,

    /// Load the call graph from a PostgreSQL database at startup
    /// (instead of requiring DATABASE_URL). The pool is dropped once
    /// the graph is loaded; the explorer holds only the in-memory graph.
    #[cfg(feature = "postgres")]
    #[arg(long)]
    postgres: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let postgres_url = {
        #[cfg(feature = "postgres")]
        {
            args.postgres
                .clone()
                .or_else(|| std::env::var("DATABASE_URL").ok())
        }
        #[cfg(not(feature = "postgres"))]
        {
            None
        }
    };

    let runtime = cognicode_runtime::Runtime::bootstrap(args.cwd, postgres_url).await?;
    let state = runtime.into_api_state();
    tracing::info!(listen = %args.listen, "starting cognicode explorer API");
    api::serve(state, args.listen).await
}
