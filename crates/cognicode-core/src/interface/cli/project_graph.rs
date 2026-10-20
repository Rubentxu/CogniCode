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
//! The walk's skips travel with the graph. A graph that does not cover
//! the whole tree has to say so, in the same words the MCP says it, and
//! not as a clean result.

use std::path::Path;
use std::sync::Arc;

use crate::application::services::analysis_service::AnalysisService;
use crate::domain::aggregates::call_graph::CallGraph;
use crate::infrastructure::graph::per_file_graph::{BuildReport, BuildStatus, SkippedFile};

/// The project's graph, and the files the walk could not read.
pub struct ProjectGraph {
    /// The graph, as the service built it.
    pub graph: Arc<CallGraph>,
    /// The files the walk skipped, if it skipped any.
    pub skipped: Vec<SkippedFile>,
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

    let skipped = match service.get_last_build_report() {
        Some(BuildReport {
            status: BuildStatus::Partial { skipped },
            ..
        }) => skipped,
        _ => Vec::new(),
    };

    Ok(ProjectGraph {
        graph: service.get_project_graph(),
        skipped,
    })
}
