//! E29 S3 lock-holder probe — opens a Database and holds it for a given duration.
//!
//! CLI args:
//!   --mode={rw,ro}  — open as read-write or read-only
//!   --path=<.lbdb>  — path to the database file
//!   --hold-secs=N    — hold for N seconds, then exit 0
//!
//! Prints "READY" to stdout once the DB is open.

use clap::Parser;
use lbug::{Database, SystemConfig};
use std::path::PathBuf;
use std::time::Duration;

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, value_name = "rw|ro", default_value = "rw")]
    mode: String,

    #[arg(long, value_name = ".lbdb")]
    path: PathBuf,

    #[arg(long, value_name = "N", default_value = "10")]
    hold_secs: u64,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let config = match args.mode.as_str() {
        "rw" => SystemConfig::default(),
        "ro" => SystemConfig::default().read_only(true),
        other => anyhow::bail!("--mode must be 'rw' or 'ro', got '{}'", other),
    };

    // Open the database
    let _db = Database::new(args.path.to_str().unwrap(), config)?;

    println!("READY");

    // Hold for the specified duration
    std::thread::sleep(Duration::from_secs(args.hold_secs));

    Ok(())
}