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
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::application::context_pack::render::{RenderView, render as render_pack};
use crate::application::context_pack::{
    AmbiguousAnchor, BudgetReport, ContextPackService, MeasureKind, Omission, PackItem,
    PackRequest, Ranked, Relevance, SelectionRequest, SnapshotView, SourceReader, Utf8UpperBound,
    measure_named,
};
use crate::application::graph_snapshot::source_digest;
use crate::domain::aggregates::call_graph::CallGraph;
use crate::infrastructure::graph::analytics::ranking_stats::RankingStats;
use crate::infrastructure::semantic::TreeSitterDeclarations;
use crate::interface::coverage::CoverageInfo;
use crate::interface::goals::GoalEnvironment;
use crate::interface::goals::common::{GoalBasis, graph_basis};
use crate::interface::project_graph::{ProjectGraph, service_with_graph};

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
    /// What the graph this answer was computed over covers.
    pub coverage: CoverageInfo,
    /// What the graph was built from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub basis: Option<GoalBasis>,
}

/// The ranker of the product, built on the first intent only.
struct IdfRelevance<'a> {
    graph: &'a CallGraph,
    stats: OnceLock<RankingStats>,
}

impl Relevance for IdfRelevance<'_> {
    fn rank(&self, intent: &str, max: usize) -> Vec<Ranked> {
        self.stats
            .get_or_init(|| RankingStats::build(self.graph))
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
pub fn pack(
    workspace: &Path,
    cache_root: Option<&Path>,
    input: &PackInput,
) -> Result<GoalPack, String> {
    let intent = input
        .intent
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());
    if input.symbols.is_empty() && intent.is_none() {
        return Err(
            "a pack needs at least one symbol or an intent: pass `symbols` or `intent`."
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

    let (_service, built) = service_with_graph(workspace, cache_root)?;
    let snapshot = snapshot_of(&built)?;
    let relevance = IdfRelevance {
        graph: &snapshot.graph,
        stats: OnceLock::new(),
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
    };
    let built_pack = service
        .build(&PackRequest {
            selection: SelectionRequest {
                anchors: input.symbols.clone(),
                intent: intent.map(str::to_string),
            },
            max_tokens,
            continuation: input.continuation_token.clone(),
        })
        .map_err(|error| error.to_string())?;

    let coverage = CoverageInfo::of(&built.coverage);
    let partial = built_pack.omitted > 0
        || !built_pack.ambiguities.is_empty()
        || !built_pack.unresolved.is_empty();
    let status = if partial || coverage.status != "complete" {
        "partial"
    } else {
        "unknown"
    };
    let summary = format!(
        "{} of {} candidate(s) within {} token(s) ({}); {} omitted.",
        built_pack.items.len(),
        built_pack.candidates,
        built_pack.budget.limit,
        measure_label(&built_pack.budget.used.kind),
        built_pack.omitted
    );
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
    })
}

fn measure_label(kind: &MeasureKind) -> String {
    crate::application::context_pack::service::measure_label(kind)
}

/// The implementation of the goal: reads its arguments and returns its answer.
pub fn run(ctx: &GoalEnvironment, arguments: &Value) -> Result<Value, String> {
    let input: PackInput = serde_json::from_value(arguments.clone())
        .map_err(|error| format!("the arguments of the goal `pack` are not valid: {error}"))?;
    let answer = pack(ctx.workspace, ctx.cache_root, &input)?;
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
    render_pack(&RenderView {
        snapshot_digest: &pack.snapshot.digest,
        items: &pack.items,
        remaining: pack.remaining,
        refused: pack.refused,
        continuation: pack.continuation_token.as_deref(),
    })
}

#[cfg(test)]
mod tests;
