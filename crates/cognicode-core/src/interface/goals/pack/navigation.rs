//! What a pack says about going further: what expanding each symbol would
//! reach, and whether the evidence delivered may be enough to stop.
//!
//! The facts are published here, where the graph is at hand, and the links
//! that act on them are derived from them by the envelope. Nothing in this
//! module chooses a tool or builds a link, and nothing here is a statement
//! about the code the pack did not select: the advisory is about the page
//! that was delivered and says so.

use serde::{Deserialize, Serialize};

use crate::application::context_pack::PackItem;
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};

/// What expanding one delivered symbol would reach.
///
/// The counts are direct edges of the snapshot's graph. A reference the graph
/// could not tie to a declaration is not counted: `coverage` says how many
/// there are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ItemReach {
    /// The identity of the symbol, as in `items`.
    pub id: String,
    /// The lines a read of its declaration returns.
    pub range_lines: u32,
    /// The symbols the graph records as calling it.
    pub direct_callers: usize,
    /// The symbols the graph records it calling.
    pub direct_callees: usize,
}

/// Something that keeps the delivered page from being enough.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdvisoryGap {
    /// What is missing: `nothing_delivered`, `candidates_left`, `refused`,
    /// `ambiguous_anchor`, `unresolved_anchor`, `change_gaps` or
    /// `walk_unfinished`.
    pub code: String,
    /// How it reads.
    pub detail: String,
}

/// Whether the page may be enough to stop at, and what it is not.
///
/// It is advice about the delivered symbols and the questions the request
/// named. It is not a certificate of coverage: `limits` is always present and
/// says what nothing assessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StopAdvisory {
    /// `may_stop` when no gap is open, `keep_going` otherwise.
    pub verdict: String,
    /// Always `advisory`: the verdict is a recommendation made from the
    /// page, and nothing checked it against the question.
    pub basis: String,
    /// What is open, empty when the verdict is `may_stop`.
    pub gaps: Vec<AdvisoryGap>,
    /// What the verdict does not say, in the words of the answer.
    pub limits: Vec<String>,
}

/// The way on from a pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PackNavigation {
    /// What expanding each delivered symbol reaches, in the order of `items`.
    pub reach: Vec<ItemReach>,
    /// Whether to stop reading.
    pub advisory: StopAdvisory,
}

/// What the request left open, as the pack counted it.
pub struct OpenGaps {
    /// Candidates left for a next page.
    pub remaining: usize,
    /// Candidates refused: changed, unreadable or not in the snapshot.
    pub refused: usize,
    /// Bare names several symbols declare.
    pub ambiguous: usize,
    /// Anchors that name no symbol.
    pub unresolved: usize,
    /// Whether the change the pack is about has gaps.
    pub change_partial: bool,
    /// Whether the walk of the graph stopped short.
    pub walk_unfinished: bool,
    /// The status of the coverage of the graph.
    pub graph_status: String,
    /// How many references the graph could not tie to a declaration.
    pub graph_pending: usize,
}

/// The way on from the page `items` delivered over `graph`.
pub fn navigation_of(items: &[PackItem], graph: &CallGraph, open: &OpenGaps) -> PackNavigation {
    PackNavigation {
        reach: items.iter().map(|item| reach_of(item, graph)).collect(),
        advisory: advise(items.len(), open),
    }
}

fn reach_of(item: &PackItem, graph: &CallGraph) -> ItemReach {
    let id = SymbolId::new(&item.id);
    ItemReach {
        id: item.id.clone(),
        range_lines: item
            .range
            .end_line
            .saturating_sub(item.range.start_line)
            .saturating_add(1),
        direct_callers: graph.callers(&id).len(),
        direct_callees: graph.callees(&id).len(),
    }
}

fn advise(delivered: usize, open: &OpenGaps) -> StopAdvisory {
    let gaps: Vec<AdvisoryGap> = [
        (delivered == 0).then(|| ("nothing_delivered", "no symbol was delivered".to_string())),
        (open.remaining > 0).then(|| {
            (
                "candidates_left",
                format!("{} candidate(s) are left for a next page", open.remaining),
            )
        }),
        (open.refused > 0).then(|| {
            (
                "refused",
                format!(
                    "{} candidate(s) were refused: their source changed, is unreadable or is not in the snapshot",
                    open.refused
                ),
            )
        }),
        (open.ambiguous > 0).then(|| {
            (
                "ambiguous_anchor",
                format!("{} name(s) are declared by several symbols", open.ambiguous),
            )
        }),
        (open.unresolved > 0).then(|| {
            (
                "unresolved_anchor",
                format!("{} anchor(s) name no symbol of the snapshot", open.unresolved),
            )
        }),
        open.change_partial
            .then(|| ("change_gaps", "the change has gaps not known".to_string())),
        open.walk_unfinished.then(|| {
            (
                "walk_unfinished",
                "the walk of the graph stopped short of what it was asked".to_string(),
            )
        }),
    ]
    .into_iter()
    .flatten()
    .map(|(code, detail)| AdvisoryGap {
        code: code.to_string(),
        detail,
    })
    .collect();
    StopAdvisory {
        verdict: if gaps.is_empty() {
            "may_stop"
        } else {
            "keep_going"
        }
        .to_string(),
        basis: "advisory".to_string(),
        gaps,
        limits: vec![
            "semantic coverage is not assessed: nothing checked that the delivered symbols are \
             everything relevant to the request"
                .to_string(),
            format!(
                "the graph reports coverage `{}` with {} reference(s) it could not tie to a \
                 declaration; the callers and callees counted are the edges it does record",
                open.graph_status, open.graph_pending
            ),
        ],
    }
}
