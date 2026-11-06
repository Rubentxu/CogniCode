//! CLI Interface - Command-line interface implementation

pub mod analyzer;
pub mod capabilities;
pub mod commands;
pub mod doctor;
/// Los enlaces del sobre, como órdenes que la terminal entiende.
pub mod follow_ups;
/// La línea de órdenes de los goals: el árbol, el parseo y la ejecución.
pub mod goal_command;
/// The names the command line publishes for the formats of the graph.
pub mod mermaid_format;
/// The project's graph, from the same authority the MCP uses.
pub mod project_graph;
// CU-1.3 — presentadores puros AnalysisOutcome → GraphResult N−1.
pub mod presenter_bridge;
/// The two channels of the CLI, and the rule that decides between them.
pub mod progress;
pub mod result;

#[cfg(feature = "evidence-cli-ladybug")]
pub mod evidence_backend;

pub use commands::{Cli, CommandExecutor, Format};
pub use goal_command::{ParsedCli, cli_command, parse_cli};
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
