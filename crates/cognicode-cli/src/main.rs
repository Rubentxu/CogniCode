//! CogniCode CLI Entry Point
//!
//! This binary provides the command-line interface for CogniCode.

use clap::Parser;
use cognicode_core::interface::cli::commands::CliCommand;
use cognicode_core::{Cli, CommandExecutor};
use rayon::ThreadPoolBuilder;
use tracing::info;

// E1.W3 — Evidence CLI adapter for the LadybugDB-backed
// `EvidenceStore`. Compiled only under `--features ladybug`; the
// default build skips this module entirely.
#[cfg(feature = "ladybug")]
mod evidence_cmd;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize logging based on verbosity
    if cli.verbose {
        // SAFETY: called before any threads are spawned
        unsafe { std::env::set_var("RUST_LOG", "debug") };
    }

    let json_capabilities = matches!(
        &cli.command,
        Some(CliCommand::Capabilities { format, json }) if *json || format == "json"
    );
    let max_level = if json_capabilities {
        tracing_subscriber::filter::LevelFilter::OFF
    } else if cli.verbose {
        tracing_subscriber::filter::LevelFilter::DEBUG
    } else {
        tracing_subscriber::filter::LevelFilter::INFO
    };
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(max_level)
        .with_target(false)
        // PRF-F1.W1 (H6): route tracing events to stderr so stdout
        // stays clean for pipelines (`cognicode analyze | jq`).
        .with_writer(std::io::stderr)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting CogniCode CLI v{}", env!("CARGO_PKG_VERSION"));

    // E1.W3 — Register the Ladybug-backed evidence backend factory
    // so `cognicode evidence list|search` can resolve to the real
    // implementation. When the CLI is built without `--features ladybug`
    // (the default), this branch compiles to a no-op.
    #[cfg(feature = "ladybug")]
    {
        if let Err(e) = evidence_cmd::register() {
            eprintln!("warning: failed to register evidence backend: {e}");
        }
    }

    // Initialize Rayon global thread pool with 8 MB stack size
    match ThreadPoolBuilder::new()
        .stack_size(8 * 1024 * 1024) // 8 MB per thread
        .build_global()
    {
        Ok(_) => info!("Rayon global thread pool initialized with 8 MB stack size"),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already been initialized") {
                tracing::warn!(
                    "Rayon global thread pool already initialized; using existing configuration"
                );
            } else {
                panic!("Failed to initialize Rayon global thread pool: {}", e);
            }
        }
    }

    // T1-5.5e: the lib's executor returns `CommandOutput` (an
    // `IndexResult`, `GraphResult`, or `Other`). The bin is now
    // responsible for picking text vs JSON and dispatching to the
    // presenter.
    //
    // `CommandOutput::Other` covers every sub-command that still
    // emits text inline (Capabilities, Doctor, FindUsages,
    // Refactor, Navigate, Analyze, ExportAssurance, Evidence,
    // DocsIngest, IssuesIngest). For those, `execute_*` has
    // already written stdout; the bin is a no-op.
    //
    // For `Index` and `Graph` (including `Graph::Full` and
    // `Graph::Mermaid` — their per-subcommand --format flag is
    // now ignored; top-level --format is canonical), the bin
    // calls `presenter::present_output` with the user-chosen
    // `cli.format`.
    //
    // B1.W2 — `cognicode identity` short-circuits before the executor
    // because it does not need CommandOutput dispatch (the identity
    // envelope comes from `cognicode_cli::identity::print` reading
    // env vars baked by build.rs). Avoiding the executor also avoids
    // cloning `CliCommand` (which is not Clone-derived).
    let cli_format = cli.format;
    if matches!(&cli.command, Some(CliCommand::Identity)) {
        let mut stdout = std::io::stdout();
        cognicode_cli::identity::print(&mut stdout)?;
        return Ok(());
    }
    let output = CommandExecutor::execute(cli).await?;

    use cognicode_core::interface::cli::doctor::DoctorStatus;
    use cognicode_core::interface::cli::result::CommandOutput;
    if !matches!(output, CommandOutput::Other) {
        let mut stdout = std::io::stdout();
        cognicode_cli::presenter::present_output(&output, cli_format, &mut stdout)?;
    }

    // T1-5.6b — `cognicode doctor` exit code. Pre-T1-5.6b the lib called
    // `std::process::exit(1)` inline for `Missing` and `exit(0)` for
    // `Ok/Warn/Info`. The migration moves the decision to the bin so
    // the lib no longer knows what an exit code is; the bin inspects
    // the typed `DoctorReport` and sets the exit code accordingly.
    // All other sub-commands exit 0 (the default).
    let exit_code: i32 = match &output {
        CommandOutput::Doctor(report) => match report.overall_status() {
            DoctorStatus::Missing => 1,
            DoctorStatus::Ok | DoctorStatus::Warn | DoctorStatus::Info => 0,
        },
        _ => 0,
    };
    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}
