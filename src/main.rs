//! CogniCode CLI Entry Point
//!
//! This binary provides the command-line interface for CogniCode.

use clap::Parser;
use cognicode::{Cli, CommandExecutor};
use rayon::ThreadPoolBuilder;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let cli = Cli::parse();

    // Initialize logging based on verbosity
    if cli.verbose {
        std::env::set_var("RUST_LOG", "debug");
    }

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .with_target(false)
        .compact()
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    info!("Starting CogniCode CLI v{}", env!("CARGO_PKG_VERSION"));

    // Initialize Rayon global thread pool with 8 MB stack size
    match ThreadPoolBuilder::new()
        .stack_size(8 * 1024 * 1024) // 8 MB per thread
        .build_global()
    {
        Ok(_) => info!("Rayon global thread pool initialized with 8 MB stack size"),
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("already been initialized") {
                tracing::warn!("Rayon global thread pool already initialized; using existing configuration");
            } else {
                panic!("Failed to initialize Rayon global thread pool: {}", e);
            }
        }
    }

    // Execute the command
    CommandExecutor::execute(cli).await?;

    Ok(())
}
