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
use crate::domain::value_objects::{PendingReference, RejectedRelation};
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
    /// The relations the repository refused, if it refused any.
    ///
    /// A relation that was refused is not an edge of the graph, so the
    /// graph's own count does not include it. Saying nothing about it
    /// would leave a reader believing the graph holds every relation the
    /// build found.
    pub rejected: Vec<RejectedRelation>,
}

/// Builds the project's graph with the service the MCP uses.
///
/// The service is the authority for what the graph contains; this
/// function only asks it and carries what it said.
///
/// The graph comes from the same decision the MCP takes: the snapshot is
/// served when it can answer for the tree, and the tree is read again
/// when it cannot. A command that builds its own graph is a command that
/// ignores what the product already calculated.
pub fn project_graph(dir: &Path) -> Result<ProjectGraph, String> {
    let cache_root = crate::application::graph_cache::default_cache_root();
    // The daemon's word, when there is a daemon to ask: a daemon that
    // watches the tree can say that the index is still fresh without the
    // product reading the content of every file again.
    let service = match &cache_root {
        Some(root) => AnalysisService::new().with_freshness_oracle(Some(std::sync::Arc::new(
            crate::interface::daemon::DaemonOracle::for_workspace(root, dir),
        ))),
        None => AnalysisService::new(),
    };
    let loaded = crate::application::graph_snapshot::load_or_build_graph(
        &service,
        dir,
        cache_root.as_deref(),
        None,
        false,
    )
    .map_err(|e| {
        format!(
            "failed to build the graph of {}: {e}",
            dir.to_string_lossy()
        )
    })?;

    // A served graph carries the holes of the tree as it is NOW —the
    // persisted graph cannot know about them— and its references and
    // relations are the graph's own, which are empty because a snapshot
    // only exists for a build that had none of them.
    let (skipped, pending, rejected) =
        if loaded.source != crate::application::analysis_basis::GraphSource::Built {
            (loaded.tree.holes, Vec::new(), Vec::new())
        } else {
            match service.get_last_build_report() {
                Some(BuildReport {
                    status:
                        BuildStatus::Partial {
                            skipped,
                            pending,
                            rejected,
                        },
                    ..
                }) => (skipped, pending, rejected),
                _ => (Vec::new(), Vec::new(), Vec::new()),
            }
        };

    Ok(ProjectGraph {
        graph: loaded.graph,
        skipped,
        pending,
        rejected,
    })
}

/// Says what the build could not cover.
///
/// The service answers with three coverages: the files the walk could not
/// read, the references it could not tie to a declaration, and the
/// relations the repository refused. A command that shows one of them and
/// hides the other tells half the truth, so every command says all of
/// them, in the same words the MCP says them.
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
    if !built.rejected.is_empty() {
        eprintln!(
            "Warning: graph is PARTIAL: {} relation(s) refused by the repository ({})",
            built.rejected.len(),
            built
                .rejected
                .iter()
                .map(|rejection| {
                    format!(
                        "{} -> {} ({})",
                        rejection.source.as_str(),
                        rejection.target.as_str(),
                        rejection.reason
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}
