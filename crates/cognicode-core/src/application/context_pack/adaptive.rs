//! The pool of an adaptive selection: who is a candidate and what is known
//! about each.
//!
//! The pool combines the signals the pack already owns —the IDF ranker, the
//! relations of the graph, Personalized PageRank over the neighbourhood the
//! walk found— under the weights of a [`SelectionPolicy`]. It decides no
//! order by itself: the service costs the best of the pool and
//! [`super::rerank`] orders them.
//!
//! What it does when the graph cannot help is part of the contract. Without
//! a relation the graph believes around the seeds the pool is the lexical
//! one, the graph signals are zero, and the report says what was missing; it
//! never draws a relation to fill the gap.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::expansion::{
    ExpansionReport, ExpansionStatus, Neighbourhood, WorkBudget, local_ppr, role_of, walk,
};
use super::policy::{Expansion, Role, SelectionPolicy, Weights};
use super::ports::Relevance;
use super::rerank::{Signals, name_terms};
use super::selection::{
    Ambiguity, Candidate, MAX_CANDIDATES, Reason, Resolution, intent_terms, rank_intent, resolve,
};
use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::value_objects::{DependencyType, SymbolKind};

/// What an adaptive request adds to a plain one.
#[derive(Debug, Clone)]
pub struct Adaptive {
    /// What the pack weighs and how.
    pub policy: SelectionPolicy,
    /// The work the walk may spend.
    pub budget: WorkBudget,
}

/// A request is a simple lookup when its intent has at most this many words
/// and the name alone answers it.
const SIMPLE_QUERY_TERMS: usize = 2;

/// A candidate with everything the selection knows about it.
#[derive(Debug, Clone)]
pub struct PoolItem {
    /// The candidate, scored with its value.
    pub candidate: Candidate,
    /// The normalised signals.
    pub signals: Signals,
    /// The part it plays.
    pub role: Role,
    /// The container that declares it, when the graph says which.
    pub container: Option<SymbolId>,
    /// The words of its name.
    pub terms: Vec<String>,
    /// Whether the caller named it.
    pub pinned: bool,
}

/// The candidates of an adaptive request, best value first.
#[derive(Debug, Clone)]
pub struct Pool {
    /// The candidates: the anchors first, then by value, ties on the id.
    pub items: Vec<PoolItem>,
    /// The bare names that resolved to more than one symbol.
    pub ambiguities: Vec<Ambiguity>,
    /// The anchors that name no symbol of the snapshot.
    pub unresolved: Vec<String>,
    /// The words the request is about.
    pub focus: Vec<String>,
    /// How the walk went.
    pub expansion: ExpansionReport,
    /// What the pool could not use, in words.
    pub gaps: Vec<String>,
    /// The weights the values were computed with: the policy's, or the
    /// lexical ones when the graph had nothing to say.
    pub weights: Weights,
}

/// What an adaptive pack publishes about how it chose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AdaptiveReport {
    /// The policy the pack was built with: version, mode and every weight.
    pub policy: SelectionPolicy,
    /// The weights the values were computed with. They equal the policy's
    /// unless the graph had nothing to say, and then only the name counts.
    pub weights_applied: Weights,
    /// How the walk of the graph went.
    pub expansion: ExpansionReport,
    /// What the selection could not use or could not know. Empty only when
    /// every signal was available; it never says the selection is complete.
    pub gaps: Vec<String>,
    /// `lexical` when the graph signals were unavailable and the order is
    /// the name's alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degraded_to: Option<String>,
    /// How many candidates the pool held before any page was cut.
    pub pool: usize,
    /// How many of them were costed and re-ordered by value per token.
    pub reranked: usize,
    /// What a score means in this pack.
    pub score_semantics: String,
}

impl AdaptiveReport {
    /// The line the text of the pack carries about how it was chosen, so a
    /// reader of the text alone is told what the structured answer says.
    pub fn notes(&self) -> Vec<String> {
        let mode = self.policy.mode.map_or("custom", |mode| mode.code());
        let mut line = format!(
            "selection: mode={mode} policy={}; expansion={}",
            self.policy.version,
            self.expansion.status.code()
        );
        if self.expansion.status != ExpansionStatus::Expanded {
            line.push_str(&format!(" ({})", self.expansion.reason));
        }
        let mut notes = vec![line];
        if let Some(degraded) = &self.degraded_to {
            notes.push(format!("selection: degraded to {degraded} signals only"));
        }
        notes
    }
}

/// The container that declares `id`, when a `Contains` relation says so.
fn container_of(graph: &CallGraph, id: &SymbolId) -> Option<SymbolId> {
    graph.callers(id).into_iter().find(|caller| {
        graph.get_symbol(caller).is_some_and(|symbol| {
            matches!(
                symbol.kind(),
                SymbolKind::Class
                    | SymbolKind::Struct
                    | SymbolKind::Trait
                    | SymbolKind::Interface
                    | SymbolKind::Enum
                    | SymbolKind::Module
                    | SymbolKind::Namespace
            )
        }) && graph
            .dependencies(caller)
            .any(|(target, dependency)| target == id && *dependency == DependencyType::Contains)
    })
}

/// Whether the walk can say something: it ran and found relations.
fn graph_speaks(report: &ExpansionReport) -> bool {
    matches!(
        report.status,
        ExpansionStatus::Expanded | ExpansionStatus::Partial | ExpansionStatus::Cancelled
    )
}

/// Why a request does not need the walk, when it does not.
fn abstention(
    policy: &SelectionPolicy,
    anchors: &[SymbolId],
    lexical: &[Candidate],
    intent: Option<&str>,
) -> Option<String> {
    match policy.expansion {
        Expansion::Never => Some("the policy does not expand".to_string()),
        Expansion::Always => None,
        Expansion::Auto => {
            let simple = anchors.is_empty()
                && intent.is_some_and(|text| intent_terms(text).len() <= SIMPLE_QUERY_TERMS)
                && lexical
                    .first()
                    .is_some_and(|first| first.reason == Reason::ExactName);
            simple.then(|| "a simple lookup: the name alone answers it".to_string())
        }
    }
}

/// The reason a neighbour of the walk is a candidate.
fn proximity_reason(
    seed: &SymbolId,
    id: &SymbolId,
    hops: u32,
    anchors: &HashSet<SymbolId>,
    found: &Neighbourhood,
) -> Reason {
    if hops == 1 && anchors.contains(seed) {
        let seed_calls = found
            .edges
            .iter()
            .any(|(source, target, _)| source == seed && target == id);
        return if seed_calls {
            Reason::CalleeOfAnchor(seed.clone())
        } else {
            Reason::CallerOfAnchor(seed.clone())
        };
    }
    Reason::Proximity {
        seed: seed.clone(),
        hops,
    }
}

/// `value / max`, or zero when nothing is positive.
fn share(value: f64, max: f64) -> f64 {
    if max > 0.0 { value / max } else { 0.0 }
}

/// Gathers the pool of `intent` and `anchors` over `graph`.
pub fn gather(
    graph: &CallGraph,
    relevance: &dyn Relevance,
    anchors: &[String],
    intent: Option<&str>,
    adaptive: &Adaptive,
) -> Pool {
    let policy = &adaptive.policy;
    let mut ambiguities = Vec::new();
    let mut unresolved = Vec::new();
    let mut named: Vec<SymbolId> = Vec::new();
    for anchor in anchors {
        match resolve(graph, anchor) {
            Resolution::Found(id) if !named.contains(&id) => named.push(id),
            Resolution::Found(_) => {}
            Resolution::Ambiguous(candidates) => ambiguities.push(Ambiguity {
                query: anchor.clone(),
                candidates,
            }),
            Resolution::Missing => unresolved.push(anchor.clone()),
        }
    }
    let lexical: Vec<Candidate> = intent
        .map(|text| rank_intent(graph, relevance, text))
        .unwrap_or_default();
    let lexical_max = lexical.iter().map(|c| c.score).fold(0.0, f64::max);
    let lexical_value: HashMap<&SymbolId, f64> = lexical
        .iter()
        .map(|c| (&c.id, share(c.score, lexical_max)))
        .collect();

    let seeds: Vec<SymbolId> = if named.is_empty() {
        lexical
            .iter()
            .take(policy.lexical_seeds)
            .map(|c| c.id.clone())
            .collect()
    } else {
        named.clone()
    };
    let mut gaps = Vec::new();
    if intent.is_none() {
        gaps.push("no_lexical_signal: the request has no intent, so names were not scored".into());
    }
    let (found, expansion) = match abstention(policy, &named, &lexical, intent) {
        Some(reason) => (
            None,
            ExpansionReport::abstained(reason, adaptive.budget.max_visits, seeds.len()),
        ),
        None if seeds.is_empty() => (
            None,
            ExpansionReport::abstained(
                "the request names no symbol and nothing matched its intent",
                adaptive.budget.max_visits,
                0,
            ),
        ),
        None => {
            let found = walk(graph, &seeds, policy, &adaptive.budget);
            let report = found.report.clone();
            (Some(found), report)
        }
    };
    if expansion.status != ExpansionStatus::Expanded {
        gaps.push(format!(
            "expansion_{}: {}",
            expansion.status.code(),
            expansion.reason
        ));
    }
    if expansion.edges_ignored > 0 {
        gaps.push(format!(
            "edges_ignored: {} relation(s) were ambiguous or below the confidence the policy asks",
            expansion.edges_ignored
        ));
    }
    if expansion.fanout_capped > 0 {
        gaps.push(format!(
            "fanout_capped: {} symbol(s) have more neighbours than the {} the walk follows",
            expansion.fanout_capped, policy.max_fanout
        ));
    }

    let usable = graph_speaks(&expansion);
    let seed_weights: HashMap<SymbolId, f64> = seeds
        .iter()
        .map(|id| {
            let weight = if named.contains(id) {
                1.0
            } else {
                lexical_value.get(id).copied().unwrap_or(0.0)
            };
            (id.clone(), weight)
        })
        .collect();
    let ppr = match (&found, usable) {
        (Some(found), true) => local_ppr(found, &seed_weights),
        _ => HashMap::new(),
    };
    let ppr_max = ppr.values().copied().fold(0.0, f64::max);

    let anchor_set: HashSet<SymbolId> = named.iter().cloned().collect();
    let seed_set: HashSet<SymbolId> = seeds.iter().cloned().collect();
    let mut proposals: Vec<Candidate> = named
        .iter()
        .map(|id| Candidate {
            id: id.clone(),
            reason: Reason::Anchor,
            score: 1.0,
        })
        .collect();
    proposals.extend(lexical.iter().cloned());
    if let (Some(found), true) = (&found, usable) {
        for id in found.nodes.iter().filter(|id| !seed_set.contains(*id)) {
            let hops = found.hops[id];
            let seed = &found.from_seed[id];
            proposals.push(Candidate {
                id: id.clone(),
                reason: proximity_reason(seed, id, hops, &anchor_set, found),
                score: 0.0,
            });
        }
    }
    let mut seen = HashSet::new();
    proposals.retain(|c| graph.get_symbol(&c.id).is_some() && seen.insert(c.id.clone()));
    proposals.truncate(MAX_CANDIDATES);

    let weights = if usable {
        policy.weights
    } else {
        Weights::LEXICAL_ONLY
    };
    let max_fan_in = proposals
        .iter()
        .map(|c| graph.fan_in(&c.id))
        .max()
        .unwrap_or(0);
    let edges = found.as_ref().map(|f| f.edges.as_slice()).unwrap_or(&[]);
    let mut items: Vec<PoolItem> = proposals
        .into_iter()
        .map(|mut candidate| {
            let id = &candidate.id;
            let pinned = anchor_set.contains(id);
            let hops = found
                .as_ref()
                .filter(|_| usable)
                .and_then(|f| f.hops.get(id).copied());
            let signals = Signals {
                lexical: if pinned {
                    1.0
                } else {
                    lexical_value.get(id).copied().unwrap_or(0.0)
                },
                ppr: share(ppr.get(id).copied().unwrap_or(0.0), ppr_max),
                proximity: match hops {
                    Some(hops) => 1.0 / (1.0 + f64::from(hops)),
                    None if anchor_set.contains(id) => 1.0,
                    None => 0.0,
                },
                impact: if usable {
                    share(
                        (1.0 + graph.fan_in(id) as f64).ln(),
                        (1.0 + max_fan_in as f64).ln(),
                    )
                } else {
                    0.0
                },
            };
            let value = signals.value(&weights);
            candidate.score = value;
            PoolItem {
                signals,
                role: role_of(graph, id, &seed_set, edges),
                container: container_of(graph, id),
                terms: graph
                    .get_symbol(id)
                    .map(|symbol| name_terms(symbol.name()))
                    .unwrap_or_default(),
                pinned,
                candidate,
            }
        })
        .collect();
    items.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| {
                if a.pinned {
                    std::cmp::Ordering::Equal
                } else {
                    b.candidate.score.total_cmp(&a.candidate.score)
                }
            })
            .then_with(|| a.candidate.id.as_str().cmp(b.candidate.id.as_str()))
    });
    let mut focus: Vec<String> = intent.map(intent_terms).unwrap_or_default();
    for seed in &named {
        if let Some(symbol) = graph.get_symbol(seed) {
            for term in name_terms(symbol.name()) {
                if !focus.contains(&term) {
                    focus.push(term);
                }
            }
        }
    }
    Pool {
        items,
        ambiguities,
        unresolved,
        focus,
        expansion,
        gaps,
        weights,
    }
}
