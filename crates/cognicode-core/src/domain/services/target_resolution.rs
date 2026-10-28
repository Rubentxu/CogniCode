//! The rule that answers "which symbol does this name mean?".
//!
//! ## Why this lives in the domain
//!
//! A name is not a piece of infrastructure: it is a fact about the code.
//! The rule that turns a name into a symbol is therefore a domain rule, and
//! it is *pure*: the same graph and the same name always give the same
//! answer — no clock, no file system, no state.
//!
//! ## The tiers
//!
//! Three, from the most specific to the least:
//!
//! 1. The **full name** — the symbol's identity in the graph
//!    (`"{file}:{name}:{line}"`). At most one symbol can carry it, because
//!    the identity is unique per graph, so this tier can never be
//!    ambiguous. It is also what a caller who knows exactly which symbol
//!    they mean writes.
//! 2. The **base name** — what the symbol is called. Several symbols can
//!    share it; those are the homonyms.
//! 3. A **segment of the full name** — the dotted pieces of the path the
//!    symbol lives in. Several symbols can share one of those too.
//!
//! The first tier that matches decides. A tier that matches exactly one
//! symbol resolves; a tier that matches several is ambiguous and reports
//! all of them; no tier matching leaves the name unresolved.
//!
//! ## Why the candidates are sorted
//!
//! The graph keeps its symbols in a hash map, so the order they come out
//! in changes from run to run. An answer that listed the candidates in that
//! order would be a different answer every time it was asked, and a
//! consumer could not tell a stable fact from an accident. Sorting by
//! identity makes the answer stable.

use crate::domain::aggregates::call_graph::{CallGraph, SymbolId};
use crate::domain::value_objects::TargetResolution;

/// Resolves a name against the symbols of a graph.
///
/// The name is matched without regard to case. The outcome is `Resolved`
/// when exactly one symbol carries it, `Ambiguous` — with every candidate —
/// when more than one does, and `Unresolved` when none does.
pub fn resolve_target(graph: &CallGraph, name: &str) -> TargetResolution {
    let query = name.to_lowercase();

    let by_identity = matching_identities(graph, &query);
    if !by_identity.is_empty() {
        return decide(query, by_identity);
    }

    let by_base_name = matching_base_names(graph, &query);
    if !by_base_name.is_empty() {
        return decide(query, by_base_name);
    }

    let by_segment = matching_segments(graph, &query);
    if !by_segment.is_empty() {
        return decide(query, by_segment);
    }

    TargetResolution::Unresolved(query)
}

/// The symbols whose full name is the query.
///
/// The full name is the symbol's identity, so there is at most one of them
/// in a graph.
fn matching_identities(graph: &CallGraph, query: &str) -> Vec<SymbolId> {
    graph
        .symbols()
        .filter(|symbol| symbol.fully_qualified_name().to_lowercase() == query)
        .map(|symbol| SymbolId::new(symbol.fully_qualified_name()))
        .collect()
}

/// The symbols whose base name is the query.
///
/// Several symbols can be called the same thing; those are the homonyms.
fn matching_base_names(graph: &CallGraph, query: &str) -> Vec<SymbolId> {
    graph
        .symbols()
        .filter(|symbol| symbol.name().to_lowercase() == query)
        .map(|symbol| SymbolId::new(symbol.fully_qualified_name()))
        .collect()
}

/// The symbols whose full name has the query as one of its segments.
fn matching_segments(graph: &CallGraph, query: &str) -> Vec<SymbolId> {
    graph
        .symbols()
        .filter(|symbol| {
            symbol
                .fully_qualified_name()
                .to_lowercase()
                .split("::")
                .flat_map(|segment| segment.split('.'))
                .any(|segment| segment == query)
        })
        .map(|symbol| SymbolId::new(symbol.fully_qualified_name()))
        .collect()
}

/// Turns a set of candidates into the outcome they describe.
///
/// The candidates are sorted by identity so that the answer does not depend
/// on the order the graph happened to be built in.
fn decide(name: String, mut candidates: Vec<SymbolId>) -> TargetResolution {
    candidates.sort();
    candidates.dedup();
    if let [only] = candidates.as_slice() {
        return TargetResolution::Resolved(only.clone());
    }
    TargetResolution::Ambiguous { name, candidates }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::aggregates::symbol::Symbol;
    use crate::domain::value_objects::{Location, SymbolKind};

    /// A graph holding one symbol per `(file, name, line)`.
    fn graph_of(symbols: &[(&str, &str, u32)]) -> CallGraph {
        let mut graph = CallGraph::new();
        for (file, name, line) in symbols {
            graph.add_symbol(Symbol::new(
                *name,
                SymbolKind::Function,
                Location::new(*file, *line, 0),
            ));
        }
        graph
    }

    #[test]
    fn a_full_name_resolves_to_the_symbol_that_carries_it() {
        let graph = graph_of(&[("src/a.rs", "helper", 10), ("src/b.rs", "helper", 20)]);

        let resolution = resolve_target(&graph, "src/a.rs:helper:10");

        assert_eq!(
            resolution,
            TargetResolution::Resolved(SymbolId::new("src/a.rs:helper:10"))
        );
    }

    #[test]
    fn a_unique_base_name_resolves() {
        let graph = graph_of(&[("src/a.rs", "helper", 10), ("src/b.rs", "other", 20)]);

        let resolution = resolve_target(&graph, "helper");

        assert_eq!(
            resolution,
            TargetResolution::Resolved(SymbolId::new("src/a.rs:helper:10"))
        );
    }

    #[test]
    fn two_symbols_with_the_same_base_name_are_ambiguous() {
        let graph = graph_of(&[("src/a.rs", "helper", 10), ("src/b.rs", "helper", 20)]);

        let resolution = resolve_target(&graph, "helper");

        match resolution {
            TargetResolution::Ambiguous { name, candidates } => {
                assert_eq!(name, "helper");
                assert_eq!(
                    candidates,
                    vec![
                        SymbolId::new("src/a.rs:helper:10"),
                        SymbolId::new("src/b.rs:helper:20"),
                    ],
                    "la ambigüedad lleva todos los candidatos"
                );
            }
            other => panic!("dos homónimos no resuelven a uno: {other:?}"),
        }
    }

    #[test]
    fn the_candidates_do_not_depend_on_the_order_the_graph_was_built_in() {
        let one = graph_of(&[("src/a.rs", "helper", 10), ("src/b.rs", "helper", 20)]);
        let other = graph_of(&[("src/b.rs", "helper", 20), ("src/a.rs", "helper", 10)]);

        assert_eq!(
            resolve_target(&one, "helper"),
            resolve_target(&other, "helper"),
            "el mismo grafo y el mismo nombre tienen que dar la misma respuesta"
        );
    }

    #[test]
    fn a_name_that_matches_nothing_is_unresolved() {
        let graph = graph_of(&[("src/a.rs", "helper", 10)]);

        assert_eq!(
            resolve_target(&graph, "ghost"),
            TargetResolution::Unresolved("ghost".to_string())
        );
    }

    #[test]
    fn the_name_is_matched_without_regard_to_case() {
        let graph = graph_of(&[("src/a.rs", "helper", 10)]);

        assert_eq!(
            resolve_target(&graph, "HELPER"),
            TargetResolution::Resolved(SymbolId::new("src/a.rs:helper:10"))
        );
    }
}
