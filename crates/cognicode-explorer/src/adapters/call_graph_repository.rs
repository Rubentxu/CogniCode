//! Adapter that implements [`SymbolRepository`] over a loaded `CallGraph`.
//!
//! The graph is held behind an `Arc` so the adapter is cheap to clone and
//! safe to share with the application service.

use std::sync::Arc;

use cognicode_core::domain::aggregates::{CallEntry, CallGraph, Symbol, SymbolId};
use cognicode_core::domain::traits::graph_query_port::{
    CalleeWithMetadata, CallerWithMetadata, EdgeWithMetadata, GraphQueryPort, RelationTarget,
    RelationTargetWithMetadata,
};
use cognicode_core::domain::value_objects::{DependencyType, Provenance};

use crate::error::{ExplorerError, ExplorerResult};
use crate::ports::symbol_repository::{
    GraphStats, ResolvedSymbol, SymbolRepository,
};

/// Adapter that exposes a `CallGraph` through the explorer port.
pub struct CallGraphRepository {
    graph: Arc<CallGraph>,
}

impl CallGraphRepository {
    pub fn new(graph: Arc<CallGraph>) -> Self {
        Self { graph }
    }

    /// Borrow the underlying graph — used by the integration test that needs
    /// to seed a graph with `add_symbol` and `add_dependency`.
    #[allow(dead_code)]
    pub(crate) fn graph_mut(&mut self) -> &mut CallGraph {
        Arc::get_mut(&mut self.graph).expect("CallGraphRepository holds the only Arc<CallGraph>")
    }

    /// Direct passthrough to [`CallGraph::callees_with_metadata`].
    ///
    /// Used by Phase 2+ explorer consumers that need to surface edge
    /// trust information (provenance, confidence) in the API. The
    /// existing [`SymbolRepository::callees`] returns plain
    /// `RelationTarget` and intentionally omits metadata to keep that
    /// trait surface stable.
    pub fn callees_with_metadata(
        &self,
        id: &SymbolId,
    ) -> Vec<(SymbolId, DependencyType, Provenance, f64)> {
        self.graph.callees_with_metadata(id)
    }

    /// Wrap an existing graph (used by `graph_mut`-style construction flows).
    pub fn from_graph(graph: CallGraph) -> Self {
        Self {
            graph: Arc::new(graph),
        }
    }
}

/// Convert a `Symbol` aggregate into a `ResolvedSymbol` DTO.
fn resolve_symbol(id: &SymbolId, graph: &CallGraph) -> Option<ResolvedSymbol> {
    graph
        .get_symbol(id)
        .map(|sym| build_resolved(id.clone(), sym))
}

/// Build a `ResolvedSymbol` directly from a `&Symbol` reference.
///
/// Used by `find_symbols_by_name` where we already have the `&Symbol`
/// returned by `CallGraph::find_by_name` and want to skip the redundant
/// `get_symbol` lookup.
fn build_resolved(id: SymbolId, sym: &Symbol) -> ResolvedSymbol {
    let loc = sym.location();
    ResolvedSymbol {
        id,
        name: sym.name().to_string(),
        kind: *sym.kind(),
        file: loc.file().to_string(),
        line: loc.line(),
        signature: sym.signature().map(|s| s.to_string()),
    }
}

fn relation_target(id: &SymbolId, graph: &CallGraph) -> Option<RelationTarget> {
    resolve_symbol(id, graph).map(|r| RelationTarget {
        id: r.id,
        name: r.name,
        kind: r.kind,
        file: r.file,
        line: r.line,
        signature: r.signature,
    })
}

impl SymbolRepository for CallGraphRepository {
    fn resolve(&self, id: &SymbolId) -> ExplorerResult<Option<ResolvedSymbol>> {
        Ok(resolve_symbol(id, &self.graph))
    }

    fn find_symbols_by_name(&self, name: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
        let resolved = self
            .graph
            .find_by_name(name)
            .into_iter()
            .map(|sym| {
                let id = SymbolId::new(sym.fully_qualified_name());
                build_resolved(id, sym)
            })
            .collect();
        Ok(resolved)
    }

    fn find_symbols_by_file(&self, file: &str) -> ExplorerResult<Vec<ResolvedSymbol>> {
        // Exact match on `location().file()` — Phase 2 deliberately avoids
        // prefix matching here so that `file:` identity resolution stays
        // predictable. The caller is responsible for any prefix semantics
        // (scope identity uses them, file identity does not).
        let mut out: Vec<ResolvedSymbol> = self
            .graph
            .symbol_ids()
            .filter(|(_, sym)| sym.location().file() == file)
            .map(|(id, sym)| build_resolved(id.clone(), sym))
            .collect();
        // Stable order: sort by line, then by name, so the "symbols in
        // this file" view renders the file's natural top-to-bottom shape.
        out.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.name.cmp(&b.name)));
        Ok(out)
    }

    fn module_list(&self) -> Vec<String> {
        let mut modules: Vec<String> = self.graph.modules().into_iter().collect();
        modules.sort();
        modules
    }

    fn all_symbols(&self) -> ExplorerResult<Vec<ResolvedSymbol>> {
        let resolved: Vec<ResolvedSymbol> = self
            .graph
            .symbol_ids()
            .map(|(id, sym)| build_resolved(id.clone(), sym))
            .collect();
        Ok(resolved)
    }

    fn graph_stats(&self) -> GraphStats {
        GraphStats {
            symbol_count: self.graph.symbol_count(),
            relation_count: self.graph.edge_count(),
        }
    }
}

impl GraphQueryPort for CallGraphRepository {
    fn callers(&self, id: &SymbolId) -> Vec<RelationTarget> {
        self.graph
            .callers(id)
            .into_iter()
            .filter_map(|caller_id| relation_target(&caller_id, &self.graph))
            .collect()
    }

    fn callees(&self, id: &SymbolId) -> Vec<RelationTarget> {
        self.graph
            .callees(id)
            .into_iter()
            .filter_map(|(callee_id, _)| relation_target(&callee_id, &self.graph))
            .collect()
    }

    fn fan_in(&self, id: &SymbolId) -> usize {
        self.graph.fan_in(id)
    }

    fn fan_out(&self, id: &SymbolId) -> usize {
        self.graph.fan_out(id)
    }

    fn callers_with_metadata(&self, id: &SymbolId) -> Vec<CallerWithMetadata> {
        self.graph
            .edges_with_metadata()
            .filter(|(_, target, _, _, _)| target == id)
            .map(|(source, _, _, provenance, confidence)| CallerWithMetadata {
                caller_id: source,
                provenance,
                confidence,
            })
            .collect()
    }

    fn callees_with_metadata(&self, id: &SymbolId) -> Vec<CalleeWithMetadata> {
        self.graph
            .callees_with_metadata(id)
            .into_iter()
            .map(|(callee_id, dependency_type, provenance, confidence)| CalleeWithMetadata {
                callee_id,
                dependency_type,
                provenance,
                confidence,
            })
            .collect()
    }

    fn dependencies_with_metadata(&self, id: &SymbolId) -> Vec<RelationTargetWithMetadata> {
        self.graph
            .dependencies_with_metadata(id)
            .map(|(target_id, dependency_type, provenance, confidence)| {
                let target =
                    relation_target(&target_id, &self.graph).unwrap_or_else(|| RelationTarget {
                        id: target_id.clone(),
                        name: String::new(),
                        kind: cognicode_core::domain::value_objects::SymbolKind::Function,
                        file: String::new(),
                        line: 0,
                        signature: None,
                    });
                RelationTargetWithMetadata {
                    target,
                    dependency_type: *dependency_type,
                    provenance,
                    confidence,
                }
            })
            .collect()
    }

    fn traverse_callees(&self, id: &SymbolId, max_depth: u8) -> Vec<CallEntry> {
        self.graph.traverse_callees(id, max_depth)
    }

    fn traverse_callers(&self, id: &SymbolId, max_depth: u8) -> Vec<CallEntry> {
        self.graph.traverse_callers(id, max_depth)
    }
}

/// Convenience constructor used by the binary that wants to fail loudly
/// when the graph has not been indexed yet.
pub fn repository_from_graph(graph: Option<Arc<CallGraph>>) -> ExplorerResult<CallGraphRepository> {
    match graph {
        Some(g) => Ok(CallGraphRepository::new(g)),
        None => Err(ExplorerError::GraphNotReady),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognicode_core::domain::aggregates::{CallGraph, Symbol, SymbolId};
    use cognicode_core::domain::value_objects::{Location, SymbolKind};

    fn build_graph() -> CallGraph {
        let mut g = CallGraph::new();
        let loc_a = Location::new("src/a.rs", 1, 0);
        let loc_b = Location::new("src/b.rs", 5, 0);
        let sym_a = Symbol::new("alpha", SymbolKind::Function, loc_a);
        let sym_b = Symbol::new("beta", SymbolKind::Function, loc_b);
        let id_a = g.add_symbol(sym_a);
        let id_b = g.add_symbol(sym_b);
        g.add_dependency(
            &id_a,
            &id_b,
            cognicode_core::domain::value_objects::DependencyType::Calls,
        )
        .expect("add dep a -> b");
        g
    }

    #[test]
    fn callees_with_metadata_passthrough_returns_metadata() {
        use cognicode_core::domain::services::ExtractionContext;

        // Build a graph with edges of mixed provenance/confidence.
        let mut g = CallGraph::new();
        let a = g.add_symbol(Symbol::new(
            "alpha",
            SymbolKind::Function,
            Location::new("src/a.rs", 1, 0),
        ));
        let b = g.add_symbol(Symbol::new(
            "beta",
            SymbolKind::Function,
            Location::new("src/b.rs", 5, 0),
        ));
        let c = g.add_symbol(Symbol::new(
            "gamma",
            SymbolKind::Function,
            Location::new("src/c.rs", 9, 0),
        ));
        g.add_dependency_with_provenance(
            &a,
            &b,
            DependencyType::Calls,
            ExtractionContext::DirectExtraction,
        )
        .expect("add a→b");
        g.add_dependency_with_provenance(
            &a,
            &c,
            DependencyType::Imports,
            ExtractionContext::Heuristic { score: 0.6 },
        )
        .expect("add a→c");

        let repo = CallGraphRepository::from_graph(g);
        let metas = repo.callees_with_metadata(&a);
        assert_eq!(metas.len(), 2);
        // Every entry must have a finite, in-range confidence.
        for (_id, _dep, _prov, conf) in &metas {
            assert!((0.0..=1.0).contains(conf));
            assert!(conf.is_finite());
            assert!(!conf.is_nan());
        }
        // Find the heuristic edge specifically.
        let heuristic = metas
            .iter()
            .find(|(_, _, prov, _)| *prov == Provenance::Inferred)
            .expect("heuristic edge");
        assert_eq!(heuristic.3, 0.6_f64);
    }

    #[test]
    fn callees_with_metadata_is_empty_for_unknown_symbol() {
        let repo = CallGraphRepository::from_graph(build_graph());
        let unknown = SymbolId::new("src/ghost.rs:ghost:1");
        assert!(repo.callees_with_metadata(&unknown).is_empty());
    }

    #[test]
    fn resolves_known_symbol() {
        let repo = CallGraphRepository::from_graph(build_graph());
        let id = SymbolId::new("src/a.rs:alpha:1");
        let resolved = repo.resolve(&id).unwrap().expect("known symbol");
        assert_eq!(resolved.name, "alpha");
        assert_eq!(resolved.file, "src/a.rs");
        assert_eq!(resolved.line, 1);
        assert!(matches!(resolved.kind, SymbolKind::Function));
    }

    #[test]
    fn returns_none_for_unknown_symbol() {
        let repo = CallGraphRepository::from_graph(build_graph());
        let id = SymbolId::new("src/missing.rs:nope:1");
        assert!(repo.resolve(&id).unwrap().is_none());
    }

    #[test]
    fn caller_and_callee_relations() {
        let repo = CallGraphRepository::from_graph(build_graph());
        let a = SymbolId::new("src/a.rs:alpha:1");
        let b = SymbolId::new("src/b.rs:beta:5");

        // a calls b → a.callees = [b], b.callers = [a]
        let a_callees = repo.callees(&a);
        let b_callers = repo.callers(&b);

        assert_eq!(a_callees.len(), 1);
        assert_eq!(a_callees[0].name, "beta");
        assert_eq!(repo.fan_out(&a), 1);
        assert_eq!(repo.fan_in(&a), 0);

        assert_eq!(b_callers.len(), 1);
        assert_eq!(b_callers[0].name, "alpha");
        assert_eq!(repo.fan_in(&b), 1);
        assert_eq!(repo.fan_out(&b), 0);
    }

    #[test]
    fn repository_from_graph_errors_when_none() {
        let result = repository_from_graph(None);
        assert!(result.is_err());
        match result.err() {
            Some(ExplorerError::GraphNotReady) => {}
            other => panic!("expected GraphNotReady, got {other:?}"),
        }
    }

    fn build_diverse_graph() -> CallGraph {
        let mut g = CallGraph::new();
        let sym_a = Symbol::new(
            "alpha",
            SymbolKind::Function,
            Location::new("src/a.rs", 1, 0),
        );
        let sym_b = Symbol::new(
            "beta",
            SymbolKind::Function,
            Location::new("src/b.rs", 5, 0),
        );
        let sym_c = Symbol::new(
            "alpha",
            SymbolKind::Struct,
            Location::new("src/c.rs", 10, 0),
        );
        let sym_d = Symbol::new(
            "gamma",
            SymbolKind::Struct,
            Location::new("src/d.rs", 20, 0),
        );
        g.add_symbol(sym_a);
        g.add_symbol(sym_b);
        g.add_symbol(sym_c);
        g.add_symbol(sym_d);
        g
    }

    #[test]
    fn find_symbols_by_name_returns_known_symbols() {
        let repo = CallGraphRepository::from_graph(build_diverse_graph());
        let results = repo
            .find_symbols_by_name("alpha")
            .expect("find_symbols_by_name ok");
        // "alpha" is a function in src/a.rs AND a struct in src/c.rs.
        assert_eq!(results.len(), 2);
        let files: Vec<&str> = results.iter().map(|r| r.file.as_str()).collect();
        assert!(files.contains(&"src/a.rs"));
        assert!(files.contains(&"src/c.rs"));
        // Every result has a non-empty name and a kind.
        for r in &results {
            assert_eq!(r.name, "alpha");
            assert!(matches!(r.kind, SymbolKind::Function | SymbolKind::Struct));
        }
    }

    #[test]
    fn find_symbols_by_name_is_case_insensitive() {
        let repo = CallGraphRepository::from_graph(build_diverse_graph());
        let upper = repo.find_symbols_by_name("ALPHA").expect("ok");
        let lower = repo.find_symbols_by_name("alpha").expect("ok");
        let mixed = repo.find_symbols_by_name("AlPhA").expect("ok");
        assert_eq!(upper.len(), 2);
        assert_eq!(lower.len(), 2);
        assert_eq!(mixed.len(), 2);
    }

    #[test]
    fn find_symbols_by_name_empty_for_missing() {
        let repo = CallGraphRepository::from_graph(build_diverse_graph());
        let results = repo
            .find_symbols_by_name("zzz_nonexistent")
            .expect("ok — must not error on missing");
        assert!(results.is_empty());
    }

    #[test]
    fn find_symbols_by_name_empty_query_returns_empty() {
        let repo = CallGraphRepository::from_graph(build_diverse_graph());
        let results = repo.find_symbols_by_name("").expect("ok");
        // `CallGraph::find_by_name("")` looks up an empty key in the index,
        // which is never populated → empty vec, not an error.
        assert!(results.is_empty());
    }

    #[test]
    fn graph_stats_reports_zero_on_empty_graph() {
        let repo = CallGraphRepository::from_graph(CallGraph::new());
        let stats = repo.graph_stats();
        assert_eq!(stats.symbol_count, 0);
        assert_eq!(stats.relation_count, 0);
    }

    #[test]
    fn graph_stats_reports_symbol_and_edge_counts() {
        let mut g = build_diverse_graph();
        // Add one edge between the two functions in the diverse graph.
        let a_id = SymbolId::new("src/a.rs:alpha:1");
        let b_id = SymbolId::new("src/b.rs:beta:5");
        g.add_dependency(
            &a_id,
            &b_id,
            cognicode_core::domain::value_objects::DependencyType::Calls,
        )
        .expect("add dep");

        let repo = CallGraphRepository::from_graph(g);
        let stats = repo.graph_stats();
        assert_eq!(stats.symbol_count, 4);
        assert_eq!(stats.relation_count, 1);
    }

    #[test]
    fn find_symbols_by_file_exact_match() {
        let repo = CallGraphRepository::from_graph(build_diverse_graph());
        let symbols = repo
            .find_symbols_by_file("src/a.rs")
            .expect("find by file ok");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "alpha");
        assert_eq!(symbols[0].file, "src/a.rs");
        assert_eq!(symbols[0].line, 1);
    }

    #[test]
    fn find_symbols_by_file_returns_empty_for_unknown_file() {
        let repo = CallGraphRepository::from_graph(build_diverse_graph());
        let symbols = repo
            .find_symbols_by_file("src/missing.rs")
            .expect("ok — empty vec for missing file");
        assert!(symbols.is_empty());
    }

    #[test]
    fn find_symbols_by_file_does_not_match_prefix() {
        // Exact match only — "src/a.rs" must NOT match a hypothetical "src/a.rs.bak".
        let mut g = CallGraph::new();
        let a = Symbol::new(
            "alpha",
            SymbolKind::Function,
            Location::new("src/a.rs", 1, 0),
        );
        let a_bak = Symbol::new(
            "alpha_bak",
            SymbolKind::Function,
            Location::new("src/a.rs.bak", 1, 0),
        );
        g.add_symbol(a);
        g.add_symbol(a_bak);
        let repo = CallGraphRepository::from_graph(g);

        let primary = repo.find_symbols_by_file("src/a.rs").expect("ok");
        assert_eq!(primary.len(), 1);
        assert_eq!(primary[0].name, "alpha");

        let suffix = repo.find_symbols_by_file("src/a.rs.bak").expect("ok");
        assert_eq!(suffix.len(), 1);
        assert_eq!(suffix[0].name, "alpha_bak");
    }

    #[test]
    fn find_symbols_by_file_results_are_sorted_by_line_then_name() {
        let mut g = CallGraph::new();
        g.add_symbol(Symbol::new(
            "zulu",
            SymbolKind::Function,
            Location::new("src/order.rs", 30, 0),
        ));
        g.add_symbol(Symbol::new(
            "alpha",
            SymbolKind::Function,
            Location::new("src/order.rs", 10, 0),
        ));
        g.add_symbol(Symbol::new(
            "mike",
            SymbolKind::Function,
            Location::new("src/order.rs", 10, 0),
        ));
        g.add_symbol(Symbol::new(
            "first",
            SymbolKind::Struct,
            Location::new("src/order.rs", 1, 0),
        ));
        let repo = CallGraphRepository::from_graph(g);

        let symbols = repo.find_symbols_by_file("src/order.rs").expect("ok");
        assert_eq!(symbols.len(), 4);
        let order: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(order, vec!["first", "alpha", "mike", "zulu"]);
    }

    #[test]
    fn module_list_returns_sorted_unique_parent_dirs() {
        let repo = CallGraphRepository::from_graph(build_diverse_graph());
        let modules = repo.module_list();
        // Symbol files: src/a.rs, src/b.rs, src/c.rs, src/d.rs
        // Parent dirs: src (×4) → deduped and sorted.
        assert_eq!(modules, vec!["src".to_string()]);
    }

    #[test]
    fn module_list_groups_nested_directories() {
        let mut g = CallGraph::new();
        g.add_symbol(Symbol::new(
            "a",
            SymbolKind::Function,
            Location::new("src/foo/x.rs", 1, 0),
        ));
        g.add_symbol(Symbol::new(
            "b",
            SymbolKind::Function,
            Location::new("src/bar/y.rs", 1, 0),
        ));
        g.add_symbol(Symbol::new(
            "c",
            SymbolKind::Function,
            Location::new("src/foo/z.rs", 1, 0),
        ));
        let repo = CallGraphRepository::from_graph(g);
        let modules = repo.module_list();
        assert_eq!(modules, vec!["src/bar".to_string(), "src/foo".to_string()]);
    }
}
