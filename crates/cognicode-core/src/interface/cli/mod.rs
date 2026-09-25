//! CLI Interface - Command-line interface implementation

pub mod commands;
pub mod doctor;

#[cfg(feature = "evidence-cli-ladybug")]
pub mod evidence_backend;

pub use commands::{Cli, CommandExecutor};

/// E1.W3 — `EvidenceCommand` is gated behind `evidence-cli-ladybug`
/// (same cfg as the variant in `CliCommand`). Re-exported here so
/// downstream crates (cli) can match on it without depending on the
/// inner `commands` module.
#[cfg(feature = "evidence-cli-ladybug")]
pub use commands::EvidenceCommand;
