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
//! 3. the direct callees and callers of the anchors;
//! 4. the most connected symbols, when the request asks for them.
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

use std::cmp::Ordering;
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::ports::Relevance;
use crate::application::services::graph_queries::hot_paths;
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};

/// The most candidates a selection holds.
pub const MAX_CANDIDATES: usize = 200;

/// The fewest callers a symbol needs to count as a hot path.
const HOT_PATH_MIN_FAN_IN: usize = 2;

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
    /// A modified line of the change falls in its declaration: `how` is
    /// `added`, `signature` or `body`.
    ChangedSymbol {
        /// How the declaration changed.
        how: String,
        /// The changed lines inside the declaration, counted from one, both
        /// ends included.
        lines: Vec<(u32, u32)>,
    },
    /// It depends on a changed symbol through an edge of the graph.
    ConsumerOfChange {
        /// The changed symbol it depends on.
        changed: SymbolId,
        /// The kind of the edge, as the graph publishes it.
        edge: String,
        /// How the edge was obtained.
        provenance: String,
        /// The confidence the edge carries.
        confidence: f64,
    },
    /// A changed symbol declares that it satisfies it.
    ContractOfChange {
        /// The changed symbol.
        changed: SymbolId,
        /// The kind of the edge, as the graph publishes it.
        edge: String,
    },
    /// It is a test the reader marked, and the graph reaches a changed
    /// symbol from it.
    TestOfChange {
        /// The changed symbol it reaches.
        changed: SymbolId,
        /// The path of the graph from the test to the changed symbol.
        path: Vec<SymbolId>,
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
            Reason::ChangedSymbol { .. } => "changed_symbol",
            Reason::ConsumerOfChange { .. } => "consumer_of_change",
            Reason::ContractOfChange { .. } => "contract_of_change",
            Reason::TestOfChange { .. } => "test_of_change",
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
            Reason::ChangedSymbol { how, lines } if lines.is_empty() => Some(how.clone()),
            Reason::ChangedSymbol { how, lines } => {
                let spans: Vec<String> = lines
                    .iter()
                    .map(|&(start, end)| {
                        if start == end {
                            start.to_string()
                        } else {
                            format!("{start}-{end}")
                        }
                    })
                    .collect();
                Some(format!("{how}; changed lines {}", spans.join(", ")))
            }
            Reason::ConsumerOfChange {
                changed,
                edge,
                provenance,
                confidence,
            } => Some(format!(
                "{edge} {changed} ({}, confidence {confidence:.2})",
                provenance.to_lowercase()
            )),
            Reason::ContractOfChange { changed, edge } => Some(format!("{changed} {edge}")),
            Reason::TestOfChange { changed, path } => {
                let hops: Vec<&str> = path.iter().map(SymbolId::as_str).collect();
                Some(format!("reaches {changed} through {}", hops.join(" -> ")))
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
    /// How many of the most connected symbols to propose when nothing else
    /// does; zero proposes none. It is the zero-query entry: no anchor and
    /// no intent, only the snapshot.
    pub hot_paths: usize,
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

/// The terms of an intent: runs of letters, digits and underscores, lowercase,
/// each once.
///
/// The ranker scores identifiers, so a phrase only finds something through
/// the words it is made of.
fn intent_terms(intent: &str) -> Vec<String> {
    let mut terms: Vec<String> = Vec::new();
    for term in intent
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|term| !term.is_empty())
        .map(str::to_lowercase)
    {
        if !terms.contains(&term) {
            terms.push(term);
        }
    }
    terms
}

/// The candidates the intent names, best first.
///
/// The ranker splits an identifier into its words and adds up what each word
/// is worth, so the terms of the intent go to it as one snake_case identifier:
/// a symbol that names several of the terms outranks one that names a single
/// common term. Ranking each term on its own would let a symbol called just
/// like one term (a field named `cache`) bury the one the phrase describes
/// (`FileFactsCache`). A symbol whose name is the intent, spelled in snake or
/// camel case, is an exact match. Ties break on the id, so the order is
/// deterministic.
fn rank_intent(graph: &CallGraph, relevance: &dyn Relevance, intent: &str) -> Vec<Candidate> {
    let terms = intent_terms(intent);
    if terms.is_empty() {
        return Vec::new();
    }
    let snake = terms.join("_");
    let camel = terms.concat();
    let mut ranked = relevance.rank(&snake, INTENT_CANDIDATES);
    ranked.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.id.as_str().cmp(b.id.as_str()))
    });
    ranked.truncate(INTENT_CANDIDATES);
    ranked
        .into_iter()
        .map(|ranked| {
            let exact = graph.get_symbol(&ranked.id).is_some_and(|symbol| {
                let name = symbol.name().to_lowercase();
                name == snake || name == camel
            });
            Candidate {
                id: ranked.id,
                reason: if exact {
                    Reason::ExactName
                } else {
                    Reason::NameRelevance
                },
                score: ranked.score,
            }
        })
        .collect()
}

/// The most connected symbols, best first.
///
/// The owner of the hot-path question is the graph queries service; this only
/// gives its rows the shape of a candidate. The score favours a symbol that
/// is called more than it calls.
fn hot_path_candidates(graph: &CallGraph, limit: usize) -> Vec<Candidate> {
    if limit == 0 {
        return Vec::new();
    }
    hot_paths(graph, limit, HOT_PATH_MIN_FAN_IN)
        .into_iter()
        .map(|row| Candidate {
            score: hotness(row.fan_in, row.fan_out),
            reason: Reason::HotPath {
                fan_in: row.fan_in,
                fan_out: row.fan_out,
            },
            id: row.id,
        })
        .collect()
}

/// How hot a symbol is, from zero to one: callers against callees.
fn hotness(fan_in: usize, fan_out: usize) -> f64 {
    match fan_in.cmp(&fan_out) {
        Ordering::Greater => fan_in as f64 / (fan_in + fan_out) as f64,
        Ordering::Equal if fan_in == 0 => 0.5,
        _ => 0.3,
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
        proposals.extend(rank_intent(graph, relevance, intent));
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

    proposals.extend(hot_path_candidates(graph, request.hot_paths));

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
