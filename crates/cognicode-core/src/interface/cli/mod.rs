//! CLI Interface - Command-line interface implementation

pub mod commands;
pub mod doctor;
/// The project's graph, from the same authority the MCP uses.
pub mod project_graph;
// CU-1.3 — presentadores puros AnalysisOutcome → GraphResult N−1.
pub mod presenter_bridge;
pub mod result;

#[cfg(feature = "evidence-cli-ladybug")]
pub mod evidence_backend;

pub use commands::{Cli, CommandExecutor, Format};
pub use result::{
    CommandOutput, DefinitionLocation, GraphError, GraphResult, HoverResult, IndexError,
    IndexResult, NavigateError, NavigateResult, ReferenceEntry,
};

/// E1.W3 — `EvidenceCommand` is gated behind `evidence-cli-ladybug`
/// (same cfg as the variant in `CliCommand`). Re-exported here so
/// downstream crates (cli) can match on it without depending on the
/// inner `commands` module.
#[cfg(feature = "evidence-cli-ladybug")]
pub use commands::EvidenceCommand;
