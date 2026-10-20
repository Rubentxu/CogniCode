//! The project's graph, from the same authority the MCP uses.
//!
//! ## Why this module exists
//!
//! The CLI and the MCP are adapters over one service. A command that
//! builds its own graph answers the same question with different facts:
//! the reader that names a Terraform resource the way the file declares
//! it, and the one that only sees an attribute of a block, are not two
//! views of the same graph — they are two graphs, and the user gets
//! whichever one the command happened to build.
//!
//! So every command that needs the project's graph asks here, and the
//! answer is the graph the MCP would hand out.
//!
//! ## What it reports
//!
//! The walk's skips travel with the graph, and so do the references the
//! graph could not tie to a declaration. A graph that does not cover the
//! whole tree, or that does not know what everything depends on, has to
//! say so, in the same words the MCP says it, and not as a clean result.

use std::path::Path;
use std::sync::Arc;

use crate::application::services::analysis_service::AnalysisService;
use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::value_objects::PendingReference;
use crate::infrastructure::graph::per_file_graph::{BuildReport, BuildStatus, SkippedFile};

/// The project's graph, and what the build could not cover.
pub struct ProjectGraph {
    /// The graph, as the service built it.
    pub graph: Arc<CallGraph>,
    /// The files the walk skipped, if it skipped any.
    pub skipped: Vec<SkippedFile>,
    /// The references the graph could not tie to a declaration, if there are
    /// any.
    pub pending: Vec<PendingReference>,
}

/// Builds the project's graph with the service the MCP uses.
///
/// The service is the authority for what the graph contains; this
/// function only asks it and carries what it said.
pub fn project_graph(dir: &Path) -> Result<ProjectGraph, String> {
    let service = AnalysisService::new();
    service.build_project_graph(dir).map_err(|e| {
        format!(
            "failed to build the graph of {}: {e}",
            dir.to_string_lossy()
        )
    })?;

    let (skipped, pending) = match service.get_last_build_report() {
        Some(BuildReport {
            status: BuildStatus::Partial { skipped, pending },
            ..
        }) => (skipped, pending),
        _ => (Vec::new(), Vec::new()),
    };

    Ok(ProjectGraph {
        graph: service.get_project_graph(),
        skipped,
        pending,
    })
}

/// Says what the build could not cover.
///
/// The service answers with two coverages: the files the walk could not
/// read, and the references it could not tie to a declaration. A command
/// that shows one of them and hides the other tells half the truth, so
/// every command says both, in the same words the MCP says them.
pub fn warn_incomplete(built: &ProjectGraph) {
    if !built.skipped.is_empty() {
        eprintln!(
            "Warning: graph is PARTIAL: {} file(s) skipped ({})",
            built.skipped.len(),
            built
                .skipped
                .iter()
                .map(|f| f.path.clone())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !built.pending.is_empty() {
        eprintln!(
            "Warning: graph is PARTIAL: {} reference(s) without a declaration ({})",
            built.pending.len(),
            built
                .pending
                .iter()
                .map(|reference| {
                    format!(
                        "{}:{} {}",
                        reference.origin_file,
                        reference.origin_line.unwrap_or_default(),
                        reference.target
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}
