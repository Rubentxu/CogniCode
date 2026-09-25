//! E2.W2 — Control Plane standalone server.
//!
//! This binary serves the single CP1 read question —
//! `GET /control-plane/workspaces/:workspace_id/architecture` —
//! against the **canonical CogniCode architecture constraints** (see
//! E2.W1: `wire_canonical_control_query()`). The state is built once
//! at startup; the source root is configurable.
//!
//! ## Why a CP1-only binary
//!
//! The full `ApiState` carries six services (workspace, search, view,
//! persistence, moldql, graph) that the architecture endpoint never
//! touches. Building all six for the architecture read question is
//! ceremony without value. `ControlPlaneState` carries only the two
//! fields the handler uses: a `ControlQueryService` and a source
//! root. The router is `control_plane_router(state)` from
//! `cognicode_explorer::api`.
//!
//! ## Fail-closed contract
//!
//! `wire_canonical_control_query()` panics at boot if the canonical
//! constraints cannot be admitted (see `control_query.rs`). That is
//! intentional — a deployment that boots with an empty registry would
//! serve `status: "incomplete"` for every query, exactly the bug E2
//! exists to close. Crashing at boot is the loud failure mode.
//!
//! ## Configuration
//!
//! CLI flags (also overridable via env for the deploys that want it):
//! * `--bind <addr>` (default `127.0.0.1:9842`): listen address.
//! * `--source-root <path>` (default `./crates/cognicode-core/src`):
//!   directory the evaluator scans for `.rs` files.
//!
//! Logs go to stderr. The server prints the bound address on startup
//! so operators can curl it.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{Context, Result};
use clap::Parser;
use cognicode_explorer::api::{ControlPlaneState, control_plane_router};
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(
    name = "cognicode-control-plane",
    about = "E2.W2 Control Plane server (CP1 read question only)"
)]
struct Args {
    /// Address to bind the HTTP server to.
    #[arg(long, env = "COGNICODE_CP_BIND", default_value = "127.0.0.1:9842")]
    bind: String,

    /// Directory the evaluator scans for `.rs` files.
    #[arg(
        long,
        env = "COGNICODE_CP_SOURCE_ROOT",
        default_value = "./crates/cognicode-core/src"
    )]
    source_root: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // tracing-subscriber initialised here so the bin controls its own
    // log format. The library is silent — observability stays in the
    // adapter (this bin), not in the core (cognicode_explorer).
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    let source_root = PathBuf::from(&args.source_root);
    if !source_root.exists() {
        anyhow::bail!(
            "source root does not exist: {} (pass --source-root <path>)",
            source_root.display()
        );
    }
    let source_root = source_root
        .canonicalize()
        .with_context(|| format!("canonicalize source root {}", source_root.display()))?;

    info!(
        bind = %args.bind,
        source_root = %source_root.display(),
        "starting cognicode-control-plane (E2.W2)"
    );

    // The wiring helper admits the 3 canonical constraints with the
    // canonical promoted admitter. Panics loudly on admission
    // rejection — see the fail-closed contract note above.
    let state = ControlPlaneState::canonical(source_root);
    info!(
        admitted = state.control_query.registry().admission.admitted().len(),
        "canonical control query wired"
    );

    let app = control_plane_router(state);
    let addr = SocketAddr::from_str(&args.bind)
        .with_context(|| format!("parse bind address {}", args.bind))?;
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind {}", addr))?;
    let bound = listener
        .local_addr()
        .with_context(|| format!("read bound addr from {}", addr))?;
    info!(bound = %bound, "listening");

    axum::serve(listener, app).await.map_err(|e| {
        error!(error = %e, "server exited with error");
        anyhow::anyhow!(e)
    })?;

    Ok(())
}
