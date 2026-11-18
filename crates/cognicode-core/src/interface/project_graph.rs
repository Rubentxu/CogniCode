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

use crate::application::analysis_basis::GraphSource;
use crate::application::graph_snapshot::LoadedGraph;
use crate::application::services::analysis_service::AnalysisService;
use crate::application::services::analyze::{Coverage, CoverageFacts};
use crate::domain::aggregates::call_graph::CallGraph;
use crate::domain::traits::graph_store::GraphStore;
use crate::domain::value_objects::file_manifest::FileManifest;
use crate::infrastructure::graph::analytics::ranking_stats::RankingStatsCache;

/// The project's graph, and what the build could not cover.
pub struct ProjectGraph {
    /// The graph, as the service built it.
    pub graph: Arc<CallGraph>,
    /// What the graph covers, and the holes it left open.
    ///
    /// The coverage is decided by the one owner of that decision —the
    /// same function the MCP and the SDK ask— so the command and the
    /// server say the same thing about the same tree. This module used
    /// to keep its own copy of the rule, and a second copy of a rule is
    /// a rule that goes stale while the first one is kept.
    pub coverage: Coverage,
    /// The sources the graph describes, when the load kept the manifest.
    ///
    /// A command that publishes the freshness of its answer reads the
    /// digest from here: the manifest the load already holds is the
    /// identity of the sources, and hashing the tree a second time would
    /// be paying for a fact the product already has.
    pub manifest: Option<FileManifest>,
    /// Where the graph came from: built now, served from the snapshot, or
    /// served from the snapshot with a live daemon vouching for it.
    pub source: GraphSource,
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
///
/// Where the snapshot is kept is what the environment says: this is the
/// entry point of a command, which owns the user's cache. A caller that
/// has been told where to keep its state uses [`service_with_graph`].
pub fn project_graph(dir: &Path) -> Result<ProjectGraph, String> {
    let cache_root = crate::application::graph_cache::default_cache_root();
    service_with_graph(dir, cache_root.as_deref()).map(|(_, project)| project)
}

/// El servicio con el grafo del workspace, y el grafo.
///
/// El servicio se queda con el grafo en su caché, así que las respuestas
/// que salen del servicio —las estadísticas, el código que no se usa— son
/// las del dueño, y no una cuenta hecha aparte.
///
/// `cache_root` is where the product keeps what it derives from `dir`;
/// `None` keeps no state and rebuilds on every call.
pub fn service_with_graph(
    dir: &Path,
    cache_root: Option<&Path>,
) -> Result<(AnalysisService, ProjectGraph), String> {
    // The daemon's word, when there is a daemon to ask: a daemon that
    // watches the tree can say that the index is still fresh without the
    // product reading the content of every file again.
    let service = match cache_root {
        Some(root) => AnalysisService::new().with_freshness_oracle(Some(std::sync::Arc::new(
            crate::interface::daemon::DaemonOracle::for_workspace(root, dir),
        ))),
        None => AnalysisService::new(),
    };
    let loaded = decide_the_graph(&service, dir, cache_root, None)?;
    Ok((service, ProjectGraph::of(loaded, None)))
}

/// Decides the graph of `dir` with `service`, and words a failure.
fn decide_the_graph(
    service: &AnalysisService,
    dir: &Path,
    cache_root: Option<&Path>,
    session_manifest: Option<&FileManifest>,
) -> Result<LoadedGraph, String> {
    crate::application::graph_snapshot::load_or_build_graph(
        service,
        dir,
        cache_root,
        session_manifest,
        false,
    )
    .map_err(|e| {
        format!(
            "failed to build the graph of {}: {e}",
            dir.to_string_lossy()
        )
    })
}

impl ProjectGraph {
    /// What a load decided, as a command carries it.
    ///
    /// A served answer carries the holes of the tree as it is NOW —the
    /// persisted graph cannot know about them— and its references and
    /// relations are the graph's own, which are empty because a snapshot
    /// only exists for a build that had none of them. The coverage is
    /// measured by the one owner of that decision, when the graph is
    /// loaded; this only carries what it said.
    ///
    /// `session_manifest` dates a graph that was served without a manifest
    /// of its own: the one the session kept when it published the graph.
    fn of(loaded: LoadedGraph, session_manifest: Option<FileManifest>) -> Self {
        Self {
            graph: loaded.graph,
            coverage: loaded.coverage,
            manifest: loaded.manifest.or(session_manifest),
            source: loaded.source,
        }
    }
}

/// What a long-lived session keeps between the calls it answers.
///
/// A one-shot command has nothing to share: the process ends with the
/// call. A server answers many calls over one tree, and three things it
/// already keeps for the rest of its tools are the ones a goal needs to
/// stop paying for the same work again: the service that holds the
/// published graph, the store that holds the manifest it was published
/// with, and the ranking statistics of that snapshot.
///
/// A goal that reads the graph through [`GraphSession::project_graph`]
/// gets the graph the session already holds for as long as the tree has not
/// changed, with the same freshness decision every other route takes, and
/// reads or rebuilds it —and publishes the new one— when the tree has
/// changed.
#[derive(Clone)]
pub struct GraphSession {
    analysis: Arc<AnalysisService>,
    store: Arc<dyn GraphStore>,
    ranking_stats: Arc<RankingStatsCache>,
}

impl GraphSession {
    /// A session over the state a server shares between its tools.
    pub fn new(
        analysis: Arc<AnalysisService>,
        store: Arc<dyn GraphStore>,
        ranking_stats: Arc<RankingStatsCache>,
    ) -> Self {
        Self {
            analysis,
            store,
            ranking_stats,
        }
    }

    /// The ranking statistics of the snapshots the session serves: one
    /// build per published graph.
    pub fn ranking_stats(&self) -> &RankingStatsCache {
        &self.ranking_stats
    }

    /// The graph of `dir` as the session holds it, or as it must be after
    /// the tree changed.
    ///
    /// The session's graph is served only when the manifest it was
    /// published with still describes the tree; otherwise the snapshot or
    /// the tree is read again, the new graph replaces the old one for every
    /// tool of the session, and its manifest is kept next to it. Between
    /// two calls over an unchanged tree the graph is the same `Arc`, which
    /// is what lets per-snapshot state be shared.
    pub fn project_graph(
        &self,
        dir: &Path,
        cache_root: Option<&Path>,
    ) -> Result<ProjectGraph, String> {
        let session_manifest = self.store.load_manifest().ok().flatten();
        let loaded = decide_the_graph(&self.analysis, dir, cache_root, session_manifest.as_ref())?;
        if let Some(manifest) = loaded.manifest.as_ref() {
            // A failed save only means the next call proves freshness
            // again, which is the safe direction.
            let _ = self.store.save_manifest(manifest);
        }
        Ok(ProjectGraph::of(loaded, session_manifest))
    }
}

impl std::fmt::Debug for GraphSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphSession").finish_non_exhaustive()
    }
}

/// How many bytes a warning about an incomplete build may occupy.
///
/// The warning is a summary: the count it publishes is exact and the
/// sample it shows is bounded. A warning that lists every hole of a
/// build is not a warning — on this repository it was 5 526 492
/// characters in a single line, and the error the person had to read sat
/// on the line after it.
///
/// The cap lives here, in the one place that writes these lines, so that
/// the second command that wants to warn does not invent its own.
pub const MAX_WARNING_BYTES: usize = 4 * 1024;

/// Says what the build could not cover.
///
/// The service answers with three coverages: the files the walk could not
/// read, the references it could not tie to a declaration, and the
/// relations the repository refused. A command that shows one of them and
/// hides the other tells half the truth, so every command says all of
/// them, in the same words the MCP says them.
pub fn warn_incomplete(built: &ProjectGraph) {
    for warning in incomplete_warnings(&built.coverage.facts()) {
        eprintln!("{warning}");
    }
}

/// The words every warning about an incomplete build opens with.
const WARNING_PREFIX: &str = "Warning: graph is PARTIAL: ";

/// The mark that says a name was cut to fit in the line.
const CUT: char = '…';

/// The warnings of a build that did not cover the tree, one line each.
///
/// The lines are a function of the facts and of nothing else: the words,
/// the counts and the sample are decided here, and the caller only
/// prints them. That is what makes them checkable — a warning that
/// nobody can read is a warning that nobody checks.
pub fn incomplete_warnings(facts: &CoverageFacts) -> Vec<String> {
    let mut warnings = Vec::new();
    if !facts.skipped.is_empty() {
        let holes = facts
            .skipped
            .iter()
            .map(|skipped| skipped.path.clone())
            .collect::<Vec<_>>();
        warnings.push(summary_of_holes("file(s) skipped", &holes));
    }
    if !facts.pending.is_empty() {
        let holes = facts
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
            .collect::<Vec<_>>();
        warnings.push(summary_of_holes(
            "reference(s) without a declaration",
            &holes,
        ));
    }
    if !facts.rejected.is_empty() {
        let holes = facts
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
            .collect::<Vec<_>>();
        warnings.push(summary_of_holes(
            "relation(s) refused by the repository",
            &holes,
        ));
    }
    warnings
}

/// Builds the warning about one kind of hole.
///
/// The count is exact and the sample is bounded by `MAX_WARNING_BYTES`:
/// a warning that says "some of them" sends the person back to the same
/// place to count them again, and a warning that lists every hole of a
/// build is a dump — on this repository it was 5 526 492 characters in
/// one line, and the error the person had to read sat on the line after
/// it.
///
/// The sample is cut at the last hole that fits, and the line says how
/// many were left out. When not even one hole fits, that one is cut
/// short and marked as cut: a warning that shows nothing of what it
/// counted says less than it knows.
fn summary_of_holes(label: &str, holes: &[String]) -> String {
    if holes.is_empty() {
        return format!("{WARNING_PREFIX}0 {label}");
    }
    let head = format!("{WARNING_PREFIX}{} {label} (", holes.len());
    // The closing bracket and the words that say what was left out.
    let close = 1;
    let tail_len = |shown: usize| -> usize {
        if shown == holes.len() {
            0
        } else {
            ", and ".len() + digits(holes.len() - shown)
        }
    };
    // The length of the sample of `shown` holes: the names and the
    // separator that goes between them.
    let mut lengths = Vec::with_capacity(holes.len() + 1);
    let mut sum = 0usize;
    lengths.push(0);
    for hole in holes {
        sum += hole.len();
        lengths.push(sum);
    }
    let sample_len = |shown: usize| -> usize {
        if shown == 0 {
            0
        } else {
            lengths[shown] + 2 * (shown - 1)
        }
    };
    // The biggest sample that fits in the cap.
    let mut shown = 0usize;
    for candidate in 0..=holes.len() {
        let total = head.len() + sample_len(candidate) + tail_len(candidate) + close;
        if total <= MAX_WARNING_BYTES {
            shown = candidate;
        }
    }
    let (sample, shown) = if shown == 0 {
        // Not even one hole fits: the first one is cut to fit, and the
        // line says that it was cut.
        let room = MAX_WARNING_BYTES
            .saturating_sub(head.len() + tail_len(1) + close)
            .saturating_sub(CUT.len_utf8());
        let cut = cut_to_fit(&holes[0], room);
        (format!("{cut}{CUT}"), 1)
    } else {
        (holes[..shown].join(", "), shown)
    };
    let tail = if shown == holes.len() {
        String::new()
    } else {
        format!(", and {} more", holes.len() - shown)
    };
    format!("{head}{sample}{tail})")
}

/// Cuts a name to the bytes that fit, on a character boundary.
fn cut_to_fit(name: &str, room: usize) -> &str {
    let mut end = name.len().min(room);
    while end > 0 && !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}

/// How many digits a number is written with.
fn digits(mut number: usize) -> usize {
    let mut count = 1;
    while number >= 10 {
        number /= 10;
        count += 1;
    }
    count
}
