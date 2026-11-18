//! The selection: which symbols are candidates, and why.
//!
//! A [`ContextSelection`] is an ephemeral projection over the frozen graph:
//! it is computed for one request, holds only identities and reasons, and is
//! dropped when the pack is built. It is not an index and it is not stored.
//!
//! Retrieval is deliberately plain. The candidates come, in this order, from
//!
//! 1. the anchors the caller named, resolved by identity;
//! 2. the symbols whose name matches the intent, ranked by the existing IDF
//!    ranker;
//! 3. the direct callees and callers of the anchors.
//!
//! A symbol that several sources propose keeps the first reason. Ties are
//! broken by symbol id, so the same snapshot and the same request give the
//! same list in every run.
//!
//! ## Identity
//!
//! An anchor is either a symbol id, which names exactly one symbol, or a bare
//! name. A bare name that several symbols declare is **not resolved**: the
//! selection reports the ambiguity with every candidate id and takes none of
//! them, because choosing one would answer about a symbol the caller may not
//! have meant.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::ports::Relevance;
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};

/// The most candidates a selection holds.
pub const MAX_CANDIDATES: usize = 200;

/// The most symbols the intent ranking contributes.
const INTENT_CANDIDATES: usize = 50;

/// Why a symbol is a candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Reason {
    /// The caller named it.
    Anchor,
    /// Its name is the intent.
    ExactName,
    /// Its name shares weighted tokens with the intent.
    NameRelevance,
    /// An anchor calls it.
    CalleeOfAnchor(SymbolId),
    /// It calls an anchor.
    CallerOfAnchor(SymbolId),
    /// It is one of the most connected symbols of the graph.
    HotPath {
        /// How many symbols call it.
        fan_in: usize,
        /// How many symbols it calls.
        fan_out: usize,
    },
}

impl Reason {
    /// The word the answer publishes.
    pub fn code(&self) -> &'static str {
        match self {
            Reason::Anchor => "anchor",
            Reason::ExactName => "exact_name",
            Reason::NameRelevance => "name_relevance",
            Reason::CalleeOfAnchor(_) => "callee_of_anchor",
            Reason::CallerOfAnchor(_) => "caller_of_anchor",
            Reason::HotPath { .. } => "hot_path",
        }
    }

    /// What the reason refers to, when it refers to something.
    pub fn detail(&self) -> Option<String> {
        match self {
            Reason::CalleeOfAnchor(anchor) | Reason::CallerOfAnchor(anchor) => {
                Some(anchor.to_string())
            }
            Reason::HotPath { fan_in, fan_out } => {
                Some(format!("fan_in={fan_in}, fan_out={fan_out}"))
            }
            _ => None,
        }
    }
}

/// A symbol proposed for the pack.
#[derive(Debug, Clone)]
pub struct Candidate {
    /// The identity of the symbol.
    pub id: SymbolId,
    /// Why it was proposed.
    pub reason: Reason,
    /// The score of the source that proposed it; comparable only within the
    /// same reason.
    pub score: f64,
}

/// A bare name that several symbols declare.
#[derive(Debug, Clone)]
pub struct Ambiguity {
    /// The name the caller wrote.
    pub query: String,
    /// Every symbol that declares it, by id.
    pub candidates: Vec<SymbolId>,
}

/// What a selection is asked for.
#[derive(Debug, Clone, Default)]
pub struct SelectionRequest {
    /// Symbol ids or bare names to anchor the pack on.
    pub anchors: Vec<String>,
    /// What the caller is trying to do, in words.
    pub intent: Option<String>,
}

/// The candidates of a request, in the order they are delivered.
#[derive(Debug, Clone, Default)]
pub struct ContextSelection {
    /// The candidates, best first, without repeats.
    pub candidates: Vec<Candidate>,
    /// The bare names that resolved to more than one symbol.
    pub ambiguities: Vec<Ambiguity>,
    /// The anchors that name no symbol of the snapshot.
    pub unresolved: Vec<String>,
}

impl ContextSelection {
    /// A selection made of candidates a caller already chose.
    ///
    /// The seeds keep their order and their reasons. It is the entry of the
    /// callers that have no intent and no anchor to resolve —the zero-query
    /// suggestion starts from the hot paths— and it is the same projection
    /// the pack builds from, so both go through one selection.
    pub fn seeded(seeds: Vec<Candidate>) -> Self {
        let mut selection = ContextSelection::default();
        let mut seen = HashSet::new();
        for seed in seeds {
            if selection.candidates.len() == MAX_CANDIDATES {
                break;
            }
            if seen.insert(seed.id.clone()) {
                selection.candidates.push(seed);
            }
        }
        selection
    }
}

/// How an anchor resolves over a graph.
enum Resolution {
    Found(SymbolId),
    Ambiguous(Vec<SymbolId>),
    Missing,
}

/// Resolves an anchor by identity first and by name second.
///
/// A name matches the symbols that declare it with the same spelling; only
/// when none does, the symbols that declare it case-insensitively are taken
/// into account, because a language that folds case declares both spellings
/// as the same name.
fn resolve(graph: &CallGraph, anchor: &str) -> Resolution {
    let as_id = SymbolId::new(anchor);
    if graph.get_symbol(&as_id).is_some() {
        return Resolution::Found(as_id);
    }
    let by_name = graph.find_by_name(anchor);
    let exact: Vec<_> = by_name
        .iter()
        .filter(|symbol| symbol.name() == anchor)
        .collect();
    let chosen: Vec<SymbolId> = if exact.is_empty() {
        by_name
            .iter()
            .map(|symbol| SymbolId::new(symbol.fully_qualified_name()))
            .collect()
    } else {
        exact
            .iter()
            .map(|symbol| SymbolId::new(symbol.fully_qualified_name()))
            .collect()
    };
    let mut ids = chosen;
    ids.sort();
    ids.dedup();
    match ids.len() {
        0 => Resolution::Missing,
        1 => Resolution::Found(ids.remove(0)),
        _ => Resolution::Ambiguous(ids),
    }
}

/// Selects the candidates of `request` over `graph`.
pub fn select(
    graph: &CallGraph,
    relevance: &dyn Relevance,
    request: &SelectionRequest,
) -> ContextSelection {
    let mut selection = ContextSelection::default();
    let mut anchors: Vec<SymbolId> = Vec::new();

    for anchor in &request.anchors {
        match resolve(graph, anchor) {
            Resolution::Found(id) => {
                if !anchors.contains(&id) {
                    anchors.push(id);
                }
            }
            Resolution::Ambiguous(candidates) => selection.ambiguities.push(Ambiguity {
                query: anchor.clone(),
                candidates,
            }),
            Resolution::Missing => selection.unresolved.push(anchor.clone()),
        }
    }

    let mut proposals: Vec<Candidate> = anchors
        .iter()
        .map(|id| Candidate {
            id: id.clone(),
            reason: Reason::Anchor,
            score: 1.0,
        })
        .collect();

    if let Some(intent) = request
        .intent
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        for ranked in relevance.rank(intent, INTENT_CANDIDATES) {
            let exact = graph
                .get_symbol(&ranked.id)
                .is_some_and(|symbol| symbol.name().eq_ignore_ascii_case(intent));
            proposals.push(Candidate {
                id: ranked.id,
                reason: if exact {
                    Reason::ExactName
                } else {
                    Reason::NameRelevance
                },
                score: ranked.score,
            });
        }
    }

    for anchor in &anchors {
        for (callee, _) in graph.callees(anchor) {
            proposals.push(Candidate {
                id: callee,
                reason: Reason::CalleeOfAnchor(anchor.clone()),
                score: 0.0,
            });
        }
        for caller in graph.callers(anchor) {
            proposals.push(Candidate {
                id: caller,
                reason: Reason::CallerOfAnchor(anchor.clone()),
                score: 0.0,
            });
        }
    }

    let mut seen = HashSet::new();
    for proposal in proposals {
        if selection.candidates.len() == MAX_CANDIDATES {
            break;
        }
        if graph.get_symbol(&proposal.id).is_some() && seen.insert(proposal.id.clone()) {
            selection.candidates.push(proposal);
        }
    }
    selection
}
