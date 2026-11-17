//! Helm adapters: read a chart from disk and, optionally, render it with the
//! official engine.
//!
//! The chart reader only reads files below the chart root and executes
//! nothing. The renderer is an optional adapter: when the `helm` binary is
//! absent it answers "unavailable" and the analysis degrades to an explicit
//! not-rendered result.

/// The name of the release a chart is rendered as, wherever it is rendered
/// for analysis.
pub const RELEASE_NAME: &str = "release";

mod chart_reader;
mod helm_cli;
mod rendered_stream;
#[cfg(feature = "evidence-kernel")]
mod revision_chart;

pub use chart_reader::{ChartReadError, read_chart, read_values_file};
pub use helm_cli::HelmCli;
pub use rendered_stream::parse_rendered_stream;
#[cfg(feature = "evidence-kernel")]
pub use revision_chart::HelmRevisionCharts;
