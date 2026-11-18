//! The goal `pack`: the symbols that answer a request, with the evidence
//! that backs each one and a budget that holds.
//!
//! The goal is a thin adapter over [`ContextPackService`]: it loads the
//! frozen snapshot, hands the service the readers that answer for the
//! source and the ranking, and publishes what the service decided. The
//! command line and the MCP reach this same function, so they cannot
//! disagree about a pack.
//!
//! ## What the answer says about itself
//!
//! - `selection` says how much of the candidate list this page delivered.
//! - `semantic_coverage` says that nothing assessed whether the candidates
//!   are everything relevant: the selection is by identity, name weight and
//!   direct relation, and a pack that delivered every candidate is still not
//!   a statement about the code it did not select.
//! - `budget` says what the limit was, what the rendered text costs and how
//!   certain that count is. Without a configured tokenizer the count is an
//!   upper bound that is never called exact.
//!
//! ## What the budget measures
//!
//! The text that `--format text` prints, header and trailer included. The
//! structured answer carries the same items with their evidence fields and
//! is not measured; the budget says so in `scope`.

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::application::context_pack::change::input::ChangeBytes;
use crate::application::context_pack::render::{RenderView, render_with_notes};
use crate::application::context_pack::{
    AmbiguousAnchor, BudgetReport, ContextPackService, MeasureKind, Omission, PackItem,
    PackRequest, Ranked, Relevance, SelectionRequest, SnapshotView, SourceReader, Utf8UpperBound,
    measure_named,
};
use crate::application::graph_snapshot::source_digest;
use crate::domain::aggregates::call_graph::CallGraph;
use crate::infrastructure::graph::analytics::ranking_stats::RankingStatsCache;
use crate::infrastructure::semantic::TreeSitterDeclarations;
use crate::interface::coverage::{
    CoverageInfo, PendingReferenceInfo, RejectedRelationInfo, SkippedFileInfo,
};
use crate::interface::goals::GoalEnvironment;
use crate::interface::goals::common::{GoalBasis, graph_basis};
use crate::interface::project_graph::{ProjectGraph, service_with_graph};

pub use self::change::ChangeReport;
use self::change::{ChangePack, build as build_change, pending_of, read_change, scope_of};

/// The budget a pack gets when the caller names none.
pub const DEFAULT_MAX_TOKENS: u64 = 2000;

/// The largest budget a caller can ask for.
pub const MAX_TOKENS_LIMIT: u64 = 200_000;

#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
/// Input for the `pack` goal
pub struct PackInput {
    /// Symbols to anchor the pack on: symbol ids (`file:name:line`, which
    /// name exactly one symbol) or bare names. A bare name that several
    /// symbols declare is reported as an ambiguity and never resolved.
    #[serde(default)]
    pub symbols: Vec<String>,
    /// What you are trying to do, in words. Symbols whose names match it are
    /// added to the pack.
    #[serde(default)]
    pub intent: Option<String>,
    /// The most tokens the rendered text may hold, under the measure the
    /// answer names. Defaults to 2000.
    #[serde(default)]
    pub max_tokens: Option<u64>,
    /// The tokenizer to count with. Only `lexical-v1` is shipped and its
    /// count is exact for itself; without it the count is a declared upper
    /// bound, never an exact count.
    #[serde(default)]
    pub tokenizer: Option<String>,
    /// The `continuation_token` of a previous answer, to get the next page
    /// of the same snapshot.
    #[serde(default)]
    pub continuation_token: Option<String>,
    /// Pack the lines of a change instead of named symbols: `staged`,
    /// `working_tree`, `commit`, `range` or `merge_candidate`. The seed is the
    /// changed lines, so `symbols` and `intent` are not passed with it.
    /// `staged` and `working_tree` take no reference; `commit` takes `head`;
    /// `range` and `merge_candidate` take `base` and `head`.
    #[serde(default)]
    pub change_scope: Option<String>,
    /// The branch a `range` is proposed against, or the branch that receives
    /// a `merge_candidate`.
    #[serde(default)]
    pub base: Option<String>,
    /// The commit of a `commit` scope, the branch that carries a `range`, or
    /// the branch merged in a `merge_candidate`.
    #[serde(default)]
    pub head: Option<String>,
}

/// How much of the candidate list a page delivered.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct SelectionReport {
    /// How many candidates this page delivered.
    pub delivered: usize,
    /// How many candidates the selection holds over all pages.
    pub candidates: usize,
    /// Whether anything of the selection is not in this page: candidates
    /// left for a next page, refused candidates, ambiguous or unresolved
    /// anchors.
    pub partial: bool,
    /// How the candidates were found.
    pub retrieval: String,
}

/// Whether anyone checked that the selection is everything relevant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct SemanticCoverage {
    /// Always `false` today: no component assesses it.
    pub assessed: bool,
    /// What that means for the reader.
    pub note: String,
}

/// The snapshot the pack was computed over.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct SnapshotInfo {
    /// The digest of the sources the graph describes.
    pub digest: String,
    /// How many files the snapshot hashed.
    pub files: usize,
}

/// How many entries of each list of the coverage the pack publishes.
const COVERAGE_LISTED: usize = 10;

/// The coverage of the graph, bounded.
///
/// The graph of a real project leaves thousands of references unresolved. The
/// budget of a pack bounds what an agent reads, so the answer carries the
/// exact totals and a short sample of each list, never the whole of them;
/// the tools that own those lists publish them in full.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
pub struct PackCoverage {
    /// The word the coverage is published with: `complete`, `partial`,
    /// `unknown` or `unsupported`.
    pub status: String,
    /// A sample of the files the walk could not read.
    pub skipped_files: Vec<SkippedFileInfo>,
    /// How many files the walk could not read.
    pub skipped_files_total: usize,
    /// A sample of the references the graph could not tie to a declaration.
    pub pending_references: Vec<PendingReferenceInfo>,
    /// How many references the graph could not tie to a declaration.
    pub pending_references_total: usize,
    /// A sample of the relations the repository refused.
    pub rejected_relations: Vec<RejectedRelationInfo>,
    /// How many relations the repository refused.
    pub rejected_relations_total: usize,
}

impl PackCoverage {
    /// The first entries of each list of `coverage`, and the exact totals.
    fn bounded(coverage: CoverageInfo) -> Self {
        Self {
            skipped_files_total: coverage.skipped_files.len(),
            pending_references_total: coverage.pending_references.len(),
            rejected_relations_total: coverage.rejected_relations.len(),
            skipped_files: coverage
                .skipped_files
                .into_iter()
                .take(COVERAGE_LISTED)
                .collect(),
            pending_references: coverage
                .pending_references
                .into_iter()
                .take(COVERAGE_LISTED)
                .collect(),
            rejected_relations: coverage
                .rejected_relations
                .into_iter()
                .take(COVERAGE_LISTED)
                .collect(),
            status: coverage.status,
        }
    }
}

/// The answer of the goal `pack`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GoalPack {
    /// One line that sums the answer up.
    pub summary: String,
    /// The version of the schema of this answer.
    pub schema_version: String,
    /// The goal that answers.
    pub goal: String,
    /// `partial` when anything of the selection is not in this page, or the
    /// graph has holes; `unknown` otherwise, because semantic coverage is
    /// not assessed. Never `complete`.
    pub status: String,
    /// The symbols of this page, in order.
    pub items: Vec<PackItem>,
    /// The candidates not delivered, listed up to a bound.
    pub omissions: Vec<Omission>,
    /// How many candidates this page did not deliver.
    pub omitted: usize,
    /// How many candidates are left for a next page.
    pub remaining: usize,
    /// How many candidates were refused.
    pub refused: usize,
    /// The index of the first candidate of this page.
    pub offset: usize,
    /// Whether candidates are left that this page did not deliver.
    pub truncated: bool,
    /// Why the page was cut: `budget` when a next page continues it, and
    /// `budget_too_small` when not even the next candidate fits, so no page
    /// could progress and only a larger `max_tokens` helps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub truncation_reason: Option<String>,
    /// The token of the next page, to pass back as `continuation_token`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub continuation_token: Option<String>,
    /// The bare names that more than one symbol declares.
    pub ambiguities: Vec<AmbiguousAnchor>,
    /// The anchors that name no symbol of the snapshot.
    pub unresolved: Vec<String>,
    /// How much of the candidate list the page delivered.
    pub selection: SelectionReport,
    /// Whether the selection was assessed against the question.
    pub semantic_coverage: SemanticCoverage,
    /// The budget and what the rendered text costs.
    pub budget: BudgetReport,
    /// The snapshot the pack is frozen on.
    pub snapshot: SnapshotInfo,
    /// What the graph this answer was computed over covers, listed up to a
    /// bound: the totals are exact, the lists are a sample.
    pub coverage: PackCoverage,
    /// What the graph was built from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis: Option<GoalBasis>,
    /// The change the pack is about: present only for a `change_scope`
    /// request, with the changed files and symbols and what is not known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<ChangeReport>,
}

/// The ranker of the product.
///
/// The statistics belong to the snapshot, not to the call: they are asked
/// from the cache on the first intent only, and the cache builds them once
/// per published graph.
struct IdfRelevance<'a> {
    graph: &'a Arc<CallGraph>,
    stats: &'a RankingStatsCache,
}

impl Relevance for IdfRelevance<'_> {
    fn rank(&self, intent: &str, max: usize) -> Vec<Ranked> {
        self.stats
            .stats_for(self.graph)
            .rank(intent, max)
            .into_iter()
            .map(|scored| Ranked {
                id: scored.symbol_id,
                score: scored.idf_score,
            })
            .collect()
    }
}

/// Reads the sources under the root of the snapshot.
struct WorkspaceSources<'a> {
    root: &'a Path,
}

impl SourceReader for WorkspaceSources<'_> {
    fn read(&self, relative: &str) -> Result<Vec<u8>, String> {
        std::fs::read(self.root.join(relative)).map_err(|error| error.to_string())
    }
}

/// The frozen snapshot of a loaded graph.
///
/// A graph that kept no manifest has no digest per file, so nothing could be
/// verified against it: it is refused instead of served unchecked.
fn snapshot_of(built: &ProjectGraph) -> Result<SnapshotView, String> {
    let manifest = built.manifest.as_ref().ok_or_else(|| {
        "the graph carries no manifest of its sources, so a pack could not be verified \
         against a snapshot; rebuild the graph"
            .to_string()
    })?;
    Ok(SnapshotView {
        graph: built.graph.clone(),
        digest: source_digest(manifest),
        graph_source: Some(built.source.published_label().to_string()),
        root: manifest.project_root.clone(),
        files: manifest
            .entries
            .iter()
            .map(|(path, entry)| {
                (
                    path.to_string_lossy().replace('\\', "/"),
                    entry.content_hash.clone(),
                )
            })
            .collect(),
    })
}

/// The pack of a request over the project's graph.
///
/// Inside a session the graph and the ranking statistics are the ones the
/// session holds for the snapshot, for as long as the tree has not changed;
/// without one, both are decided for this call alone. The answer is the same
/// either way.
pub fn pack(environment: &GoalEnvironment, input: &PackInput) -> Result<GoalPack, String> {
    let GoalEnvironment {
        workspace,
        cache_root,
        session,
    } = *environment;
    let intent = input
        .intent
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    let scope = scope_of(input)?;
    if scope.is_none() && input.symbols.is_empty() && intent.is_none() {
        return Err(
            "a pack needs at least one symbol, an intent or a change scope: pass `symbols`, \
             `intent` or `change_scope`."
                .to_string(),
        );
    }
    let max_tokens = input.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS);
    if max_tokens > MAX_TOKENS_LIMIT {
        return Err(format!(
            "`max_tokens` is {max_tokens} and the most a pack can be asked for is {MAX_TOKENS_LIMIT}."
        ));
    }
    let measure: Box<dyn crate::application::context_pack::TokenMeasure> = match input
        .tokenizer
        .as_deref()
    {
        None => Box::new(Utf8UpperBound),
        Some(name) => measure_named(name).ok_or_else(|| {
            format!(
                "`{name}` is not a tokenizer this build ships: the one it ships is `lexical-v1`."
            )
        })?,
    };

    let change = scope
        .as_ref()
        .map(|scope| read_change(workspace, scope))
        .transpose()?;
    let token_scope = change.as_ref().map(ChangeBytes::digest).unwrap_or_default();

    let built = match session {
        Some(session) => session.project_graph(workspace, cache_root)?,
        None => service_with_graph(workspace, cache_root)?.1,
    };
    let snapshot = snapshot_of(&built)?;
    let one_call = RankingStatsCache::default();
    let relevance = IdfRelevance {
        graph: &snapshot.graph,
        stats: session.map_or(&one_call, |session| session.ranking_stats()),
    };
    let sources = WorkspaceSources {
        root: &snapshot.root,
    };
    let service = ContextPackService {
        snapshot: &snapshot,
        relevance: &relevance,
        sources: &sources,
        declarations: &TreeSitterDeclarations,
        measure: measure.as_ref(),
        token_scope: &token_scope,
    };
    let coverage = CoverageInfo::of(&built.coverage);
    let pending = pending_of(&coverage.pending_references);
    let (built_pack, change_report) = match (&scope, &change) {
        (Some(scope), Some(change)) => {
            let ChangePack { page, report } = build_change(
                &service,
                scope,
                change,
                &pending,
                input.continuation_token.as_deref(),
                max_tokens,
            )
            .map_err(|error| error.to_string())?;
            (page, Some(report))
        }
        _ => {
            let page = service
                .build(&PackRequest {
                    selection: SelectionRequest {
                        anchors: input.symbols.clone(),
                        intent: intent.map(str::to_string),
                        hot_paths: 0,
                    },
                    max_tokens,
                    continuation: input.continuation_token.clone(),
                })
                .map_err(|error| error.to_string())?;
            (page, None)
        }
    };

    let coverage = PackCoverage::bounded(coverage);
    let partial = built_pack.omitted > 0
        || !built_pack.ambiguities.is_empty()
        || !built_pack.unresolved.is_empty()
        || change_report.as_ref().is_some_and(ChangeReport::is_partial);
    let status = if partial || coverage.status != "complete" {
        "partial"
    } else {
        "unknown"
    };
    let mut summary = format!(
        "{} of {} candidate(s) within {} token(s) ({}); {} omitted.",
        built_pack.items.len(),
        built_pack.candidates,
        built_pack.budget.limit,
        measure_label(&built_pack.budget.used.kind),
        built_pack.omitted
    );
    if let Some(change) = &change_report {
        summary = format!(
            "{} change: {} file(s), {} changed symbol(s), {} gap(s) not known; {summary}",
            change.scope,
            change.files_total,
            change.symbols.len(),
            change.gaps_total
        );
    }
    let truncated = built_pack.remaining > 0;
    let truncation_reason = truncated.then(|| {
        if built_pack.continuation_token.is_some() {
            "budget"
        } else {
            "budget_too_small"
        }
        .to_string()
    });
    Ok(GoalPack {
        summary,
        schema_version: "cognicode.goal.pack/v1".to_string(),
        goal: "pack".to_string(),
        status: status.to_string(),
        selection: SelectionReport {
            delivered: built_pack.items.len(),
            candidates: built_pack.candidates,
            partial,
            retrieval: "anchors, name relevance (IDF) and direct relations".to_string(),
        },
        semantic_coverage: SemanticCoverage {
            assessed: false,
            note: "nothing assessed whether the selection is everything relevant to the \
                   request; it is a selection by identity, name weight and direct relation"
                .to_string(),
        },
        snapshot: SnapshotInfo {
            digest: snapshot.digest.clone(),
            files: snapshot.files.len(),
        },
        omissions: built_pack.omissions,
        omitted: built_pack.omitted,
        remaining: built_pack.remaining,
        refused: built_pack.refused,
        offset: built_pack.offset,
        truncated,
        truncation_reason,
        continuation_token: built_pack.continuation_token,
        ambiguities: built_pack.ambiguities,
        unresolved: built_pack.unresolved,
        budget: built_pack.budget,
        items: built_pack.items,
        coverage,
        basis: graph_basis(workspace, &built),
        change: change_report,
    })
}

fn measure_label(kind: &MeasureKind) -> String {
    crate::application::context_pack::service::measure_label(kind)
}

/// The implementation of the goal: reads its arguments and returns its answer.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let input: PackInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `pack` are not valid: {error}"))?;
    let answer = pack(ctx, &input)?;
    serde_json::to_value(answer)
        .map_err(|error| format!("the answer of the goal `pack` is not serialisable: {error}"))
}

/// The answer of the goal `pack` as text.
///
/// The text is the one the budget was measured on: the same renderer, over
/// the same items.
pub fn render(answer: &Value) -> String {
    let pack: GoalPack = serde_json::from_value(answer.clone())
        .expect("the answer of the goal `pack` is the answer its own type describes");
    let notes = pack
        .change
        .as_ref()
        .map(|change| change.notes.as_slice())
        .unwrap_or_default();
    render_with_notes(
        &RenderView {
            snapshot_digest: &pack.snapshot.digest,
            items: &pack.items,
            remaining: pack.remaining,
            refused: pack.refused,
            continuation: pack.continuation_token.as_deref(),
        },
        notes,
    )
}

mod change;

#[cfg(test)]
mod change_tests;
#[cfg(test)]
mod tests;
