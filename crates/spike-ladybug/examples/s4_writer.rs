//! E29 S4 crash-recovery probe — validates lbug 0.19.0 WAL durability.
//!
//! Three modes:
//!   --mode=clean:           open DB, write N rows via UNWIND, drop cleanly (exit 0)
//!   --mode=crash:           same as clean, but print READY then park indefinitely
//!   --mode=crash-pre-write: create table only, print READY, park (no rows written)
//!
//! After --mode=crash:
//!   - SIGKILL the process (no Drop runs)
//!   - Reopen with Database::new → WAL is auto-replayed
//!   - Assert committed data recovered or zero (no partial/corrupt)

use clap::Parser;
use lbug::{Connection, Database, SystemConfig};
use std::path::PathBuf;

#[derive(clap::Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(long, value_name = "clean|crash|crash-pre-write", default_value = "clean")]
    mode: String,

    #[arg(long, value_name = ".lbdb")]
    path: PathBuf,

    #[arg(long, value_name = "N", default_value = "1000")]
    rows: i64,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let path_str = args.path.to_str().unwrap();

    match args.mode.as_str() {
        "clean" => {
            let db = Database::new(path_str, SystemConfig::default())?;
            let conn = Connection::new(&db)?;

            // Create node table
            conn.query("CREATE NODE TABLE IF NOT EXISTS Probe(id INT64, PRIMARY KEY(id));")?;

            // Write N rows via UNWIND
            conn.query(&format!(
                "UNWIND range(1, {}) AS i CREATE (n:Probe {{id: i}});",
                args.rows
            ))?;

            // db drops here → force_checkpoint_on_close runs (clean exit)
            println!("DONE");
        }

        "crash" => {
            let db = Database::new(path_str, SystemConfig::default())?;
            let conn = Connection::new(&db)?;

            // Create node table
            conn.query("CREATE NODE TABLE IF NOT EXISTS Probe(id INT64, PRIMARY KEY(id));")?;

            // Write N rows via UNWIND
            conn.query(&format!(
                "UNWIND range(1, {}) AS i CREATE (n:Probe {{id: i}});",
                args.rows
            ))?;

            // Signal that rows are committed to WAL
            println!("READY");

            // Park indefinitely — no Drop runs on SIGKILL
            std::thread::park();
        }

        "crash-pre-write" => {
            let db = Database::new(path_str, SystemConfig::default())?;
            let conn = Connection::new(&db)?;

            // Create node table only (no rows written)
            conn.query("CREATE NODE TABLE IF NOT EXISTS Probe(id INT64, PRIMARY KEY(id));")?;

            // Signal ready before any data written
            println!("READY");

            // Park indefinitely — no Drop runs on SIGKILL
            std::thread::park();
        }

        other => {
            anyhow::bail!("--mode must be 'clean', 'crash', or 'crash-pre-write', got '{}'", other);
        }
    }

    Ok(())
}
