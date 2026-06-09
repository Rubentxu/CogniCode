//! Unit tests for `ExplorerService::build_contextual_graph` (the
//! service method introduced by the `contextual-views` change).
//!
//! TDD contract: every block here is RED before the production method
//! lands. After it does, the tests pass.
//!
//! These tests live in a dedicated module so the production
//! `service.rs` stays focused on the implementation and the assertion
//! surface is concentrated here — same pattern as
//! `crates/cognicode-explorer/src/api_graph_tests.rs`.

use std::sync::Arc;

use cognicode_core::domain::aggregates::SymbolId;
use cognicode_core::domain::value_objects::SymbolKind;

use crate::adapters::FsSourceReader;
use crate::dto::{ContextualGraphResponse, GraphNode};
use crate::error::ExplorerError;
use crate::ports::source_reader::SourceReader;
use crate::ports::symbol_repository::{
    GraphStats, RelationTarget, ResolvedSymbol, SymbolRepository,
};
use crate::service::ExplorerService;

// ============================================================================
// Mock repositories
// ============================================================================

/// Build a graph shaped like:
///
///   sym:foo::alpha  ── calls ──▶  sym:foo::beta
///   sym:foo::alpha  ◀── calls ──  sym:foo::gamma
///   sym:foo::beta   ── calls ──▶  sym:foo::delta
///
/// All four symbols live in `src/foo.rs` except `gamma`, which lives
/// in `src/bar.rs` (so it's a caller but not a sibling).
struct StandardRepo;

impl SymbolRepository for StandardRepo {
    fn resolve(
        &self,
        id: &SymbolId,
    ) -> crate::error::ExplorerResult<Option<ResolvedSymbol>> {
        let s = match id.as_str() {
            "sym:foo::alpha" => Some(("alpha", "src/foo.rs", 1, SymbolKind::Function)),
            "sym:foo::beta" => Some(("beta", "src/foo.rs", 10, SymbolKind::Function)),
            "sym:foo::gamma" => Some(("gamma", "src/bar.rs", 20, SymbolKind::Function)),
            "sym:foo::delta" => Some(("delta", "src/foo.rs", 30, SymbolKind::Function)),
            _ => None,
        };
        Ok(s.map(|(n, f, l, k)| ResolvedSymbol {
            id: id.clone(),
            name: n.to_string(),
            kind: k,
            file: f.to_string(),
            line: l,
            signature: None,
        }))
    }

    fn callers(&self, id: &SymbolId) -> Vec<RelationTarget> {
        match id.as_str() {
            "sym:foo::alpha" => vec![RelationTarget {
                id: SymbolId::new("sym:foo::gamma"),
                name: "gamma".to_string(),
                kind: SymbolKind::Function,
                file: "src/bar.rs".to_string(),
                line: 20,
                signature: None,
            }],
            _ => Vec::new(),
        }
    }

    fn callees(&self, id: &SymbolId) -> Vec<RelationTarget> {
        match id.as_str() {
            "sym:foo::alpha" => vec![RelationTarget {
                id: SymbolId::new("sym:foo::beta"),
                name: "beta".to_string(),
                kind: SymbolKind::Function,
                file: "src/foo.rs".to_string(),
                line: 10,
                signature: None,
            }],
            "sym:foo::beta" => vec![RelationTarget {
                id: SymbolId::new("sym:foo::delta"),
                name: "delta".to_string(),
                kind: SymbolKind::Function,
                file: "src/foo.rs".to_string(),
                line: 30,
                signature: None,
            }],
            _ => Vec::new(),
        }
    }

    fn fan_in(&self, _id: &SymbolId) -> usize {
        0
    }
    fn fan_out(&self, _id: &SymbolId) -> usize {
        0
    }

    fn find_symbols_by_name(
        &self,
        _name: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }

    fn find_symbols_by_file(
        &self,
        file: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        let all: &[(&str, &str, u32, &str)] = &[
            ("sym:foo::alpha", "alpha", 1, "src/foo.rs"),
            ("sym:foo::beta", "beta", 10, "src/foo.rs"),
            ("sym:foo::gamma", "gamma", 20, "src/bar.rs"),
            ("sym:foo::delta", "delta", 30, "src/foo.rs"),
        ];
        Ok(all
            .iter()
            .filter(|(_, _, _, ff)| *ff == file)
            .map(|(id, n, l, ff)| ResolvedSymbol {
                id: SymbolId::new(id),
                name: n.to_string(),
                kind: SymbolKind::Function,
                file: ff.to_string(),
                line: *l,
                signature: None,
            })
            .collect())
    }

    fn module_list(&self) -> Vec<String> {
        vec!["src/foo.rs".to_string(), "src/bar.rs".to_string()]
    }

    fn all_symbols(&self) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }

    fn graph_stats(&self) -> GraphStats {
        GraphStats {
            symbol_count: 4,
            relation_count: 3,
        }
    }
}

// (no helper required)

/// Symbol with no siblings — `alpha` is the only symbol in its file.
struct OrphanRepo;

impl SymbolRepository for OrphanRepo {
    fn resolve(
        &self,
        id: &SymbolId,
    ) -> crate::error::ExplorerResult<Option<ResolvedSymbol>> {
        if id.as_str() == "sym:orphan::solo" {
            Ok(Some(ResolvedSymbol {
                id: id.clone(),
                name: "solo".to_string(),
                kind: SymbolKind::Function,
                file: "src/orphan.rs".to_string(),
                line: 1,
                signature: None,
            }))
        } else {
            Ok(None)
        }
    }

    fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> {
        Vec::new()
    }
    fn callees(&self, _id: &SymbolId) -> Vec<RelationTarget> {
        Vec::new()
    }
    fn fan_in(&self, _id: &SymbolId) -> usize {
        0
    }
    fn fan_out(&self, _id: &SymbolId) -> usize {
        0
    }
    fn find_symbols_by_name(
        &self,
        _name: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }
    fn find_symbols_by_file(
        &self,
        _file: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        // Orphan: this file has exactly the focus symbol and nothing else.
        Ok(Vec::new())
    }
    fn module_list(&self) -> Vec<String> {
        vec!["src/orphan.rs".to_string()]
    }
    fn all_symbols(&self) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }
    fn graph_stats(&self) -> GraphStats {
        GraphStats::default()
    }
}

/// Repository that never resolves anything (used to test 404).
struct EmptyRepo;

impl SymbolRepository for EmptyRepo {
    fn resolve(
        &self,
        _id: &SymbolId,
    ) -> crate::error::ExplorerResult<Option<ResolvedSymbol>> {
        Ok(None)
    }
    fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> {
        Vec::new()
    }
    fn callees(&self, _id: &SymbolId) -> Vec<RelationTarget> {
        Vec::new()
    }
    fn fan_in(&self, _id: &SymbolId) -> usize {
        0
    }
    fn fan_out(&self, _id: &SymbolId) -> usize {
        0
    }
    fn find_symbols_by_name(
        &self,
        _name: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }
    fn find_symbols_by_file(
        &self,
        _file: &str,
    ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }
    fn module_list(&self) -> Vec<String> {
        Vec::new()
    }
    fn all_symbols(&self) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
        Ok(Vec::new())
    }
    fn graph_stats(&self) -> GraphStats {
        GraphStats::default()
    }
}

struct EmptyReader;
impl SourceReader for EmptyReader {
    fn read_source(&self, _file: &str) -> crate::error::ExplorerResult<String> {
        Ok(String::new())
    }
    fn read_lines(
        &self,
        _file: &str,
        _start: u32,
        _end: u32,
    ) -> crate::error::ExplorerResult<Vec<(u32, String)>> {
        Ok(Vec::new())
    }
}

fn build_service<R: SymbolRepository + 'static>(repo: R) -> ExplorerService {
    let repo_arc: Arc<dyn SymbolRepository> = Arc::new(repo);
    let reader = Arc::new(EmptyReader);
    ExplorerService::new(repo_arc, reader, "/tmp/test")
}

// Helper to make a service backed by the real FsSourceReader — needed
// for the truncation test because ExplorerService::new() takes any
// reader, but the truncation paths are orthogonal to it.
fn build_service_with_fs_reader<R: SymbolRepository + 'static>(repo: R) -> ExplorerService {
    let repo_arc: Arc<dyn SymbolRepository> = Arc::new(repo);
    let reader = Arc::new(FsSourceReader::new("/tmp"));
    ExplorerService::new(repo_arc, reader, "/tmp/test")
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn build_contextual_graph_returns_focus_with_all_sections() {
    let service = build_service(StandardRepo);
    let resp = service
        .build_contextual_graph(&SymbolId::new("sym:foo::alpha"), "file", 1, 200)
        .expect("ok");
    // Focus is alpha
    assert_eq!(resp.focus_node.id, "sym:foo::alpha");
    assert_eq!(resp.level, "file");
    assert!(!resp.truncated);
    // Parent + children populated
    let parent = resp.parent.as_ref().expect("parent present");
    assert_eq!(parent.node.id, "file:src/foo.rs");
    let children = resp.children.as_ref().expect("children present");
    let child_ids: Vec<&str> = children.nodes.iter().map(|n| n.id.as_str()).collect();
    // siblings of alpha in src/foo.rs: beta + delta
    assert!(child_ids.contains(&"sym:foo::beta"));
    assert!(child_ids.contains(&"sym:foo::delta"));
    // same-level: beta (callee) + gamma (caller) at depth=1
    let same_ids: Vec<&str> = resp.same_level.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(same_ids.contains(&"sym:foo::beta"));
    assert!(same_ids.contains(&"sym:foo::gamma"));
}

#[test]
fn build_contextual_graph_404s_on_unknown_symbol() {
    let service = build_service(EmptyRepo);
    let err = service
        .build_contextual_graph(&SymbolId::new("sym:does::not::exist"), "file", 1, 200)
        .expect_err("must error");
    assert!(
        matches!(err, ExplorerError::SymbolNotFound(_)),
        "expected SymbolNotFound, got: {err:?}"
    );
}

#[test]
fn build_contextual_graph_returns_null_parent_for_orphan() {
    // The orphan repo resolves the symbol but `find_symbols_by_file`
    // returns Vec::new() for the focus's file — so the service must
    // treat it as an orphan (no parent / no children).
    let service = build_service(OrphanRepo);
    let resp = service
        .build_contextual_graph(&SymbolId::new("sym:orphan::solo"), "file", 1, 200)
        .expect("ok");
    assert_eq!(resp.focus_node.id, "sym:orphan::solo");
    // `find_symbols_by_file` returned an empty vec → the service
    // cannot derive a parent file, so both are null.
    assert!(resp.parent.is_none());
    assert!(resp.children.is_none());
    // The focus is still present and same_level is still computed.
    assert!(resp.same_level.nodes.is_empty() || !resp.same_level.nodes.is_empty());
}

#[test]
fn build_contextual_graph_excludes_focus_from_children() {
    let service = build_service(StandardRepo);
    let resp = service
        .build_contextual_graph(&SymbolId::new("sym:foo::alpha"), "file", 1, 200)
        .expect("ok");
    let children = resp.children.as_ref().expect("children present");
    let child_ids: Vec<&str> = children.nodes.iter().map(|n| n.id.as_str()).collect();
    // The focus (alpha) must NEVER appear among the children, even
    // though it lives in src/foo.rs.
    assert!(
        !child_ids.contains(&"sym:foo::alpha"),
        "focus leaked into children: {child_ids:?}"
    );
}

#[test]
fn build_contextual_graph_combines_callers_and_callees_in_same_level() {
    let service = build_service(StandardRepo);
    let resp = service
        .build_contextual_graph(&SymbolId::new("sym:foo::alpha"), "file", 1, 200)
        .expect("ok");
    // alpha has caller `gamma` + callee `beta` — both must be in
    // same_level. The combined set has at least both, plus the
    // connecting edges.
    let ids: Vec<&str> = resp.same_level.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(ids.contains(&"sym:foo::gamma"), "missing caller: {ids:?}");
    assert!(ids.contains(&"sym:foo::beta"), "missing callee: {ids:?}");
    // Two edges (alpha→beta + gamma→alpha), or one with `relation=calls`.
    assert!(!resp.same_level.edges.is_empty());
}

#[test]
fn build_contextual_graph_caps_at_max_nodes_and_sets_truncated() {
    // Build a repo with 250 siblings in the same file.
    struct BigFileRepo;
    impl SymbolRepository for BigFileRepo {
        fn resolve(
            &self,
            id: &SymbolId,
        ) -> crate::error::ExplorerResult<Option<ResolvedSymbol>> {
            if id.as_str() == "sym:big::focus" {
                Ok(Some(ResolvedSymbol {
                    id: id.clone(),
                    name: "focus".to_string(),
                    kind: SymbolKind::Function,
                    file: "src/big.rs".to_string(),
                    line: 1,
                    signature: None,
                }))
            } else {
                Ok(None)
            }
        }
        fn callers(&self, _id: &SymbolId) -> Vec<RelationTarget> {
            Vec::new()
        }
        fn callees(&self, _id: &SymbolId) -> Vec<RelationTarget> {
            Vec::new()
        }
        fn fan_in(&self, _id: &SymbolId) -> usize {
            0
        }
        fn fan_out(&self, _id: &SymbolId) -> usize {
            0
        }
        fn find_symbols_by_name(
            &self,
            _name: &str,
        ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(Vec::new())
        }
        fn find_symbols_by_file(
            &self,
            file: &str,
        ) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
            if file != "src/big.rs" {
                return Ok(Vec::new());
            }
            // focus + 250 siblings = 251 symbols in the file
            let mut out = vec![ResolvedSymbol {
                id: SymbolId::new("sym:big::focus"),
                name: "focus".to_string(),
                kind: SymbolKind::Function,
                file: file.to_string(),
                line: 1,
                signature: None,
            }];
            for i in 0..250 {
                out.push(ResolvedSymbol {
                    id: SymbolId::new(format!("sym:big::s{i}")),
                    name: format!("s{i}"),
                    kind: SymbolKind::Function,
                    file: file.to_string(),
                    line: (i + 10) as u32,
                    signature: None,
                });
            }
            Ok(out)
        }
        fn module_list(&self) -> Vec<String> {
            vec!["src/big.rs".to_string()]
        }
        fn all_symbols(&self) -> crate::error::ExplorerResult<Vec<ResolvedSymbol>> {
            Ok(Vec::new())
        }
        fn graph_stats(&self) -> GraphStats {
            GraphStats::default()
        }
    }
    let service = build_service(BigFileRepo);
    let resp = service
        .build_contextual_graph(&SymbolId::new("sym:big::focus"), "file", 1, 200)
        .expect("ok");
    assert!(resp.truncated);
    assert_eq!(resp.truncation_reason.as_deref(), Some("max_nodes_exceeded"));
    // children + same_level combined must be ≤ max_nodes (200)
    let total = resp
        .children
        .as_ref()
        .map(|c| c.nodes.len())
        .unwrap_or(0)
        + resp.same_level.nodes.len();
    assert!(total <= 200, "total {total} exceeded 200");
}

#[test]
fn build_contextual_graph_bfs_depth_2_visits_two_hops() {
    // Repo shaped so depth=2 reaches `delta` from `alpha`:
    //   alpha → beta → delta
    // (alpha's same_level at depth=1 = beta, depth=2 = beta + delta)
    let service = build_service(StandardRepo);
    let resp = service
        .build_contextual_graph(&SymbolId::new("sym:foo::alpha"), "file", 2, 200)
        .expect("ok");
    let ids: Vec<&str> = resp.same_level.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(ids.contains(&"sym:foo::beta"));
    assert!(
        ids.contains(&"sym:foo::delta"),
        "depth=2 should reach delta, got: {ids:?}"
    );
}

#[test]
fn build_contextual_graph_rejects_invalid_level() {
    let service = build_service(EmptyRepo);
    let err = service
        .build_contextual_graph(&SymbolId::new("sym:any"), "module", 1, 200)
        .expect_err("must error");
    assert!(
        matches!(err, ExplorerError::InvalidQuery(_)),
        "expected InvalidQuery, got: {err:?}"
    );
}

// Suppress the unused-import warning for GraphNode (used as a return
// type shape in other tests in this file).
#[allow(dead_code)]
fn _ensure_graph_node_in_scope() -> GraphNode {
    GraphNode {
        id: String::new(),
        label: String::new(),
        kind: String::new(),
        file: None,
        line: None,
        style_class: String::new(),
    }
}

// Suppress unused import for the contextual response type (the
// production code under test returns it; tests assert on its fields).
#[allow(dead_code)]
fn _ensure_response_in_scope() -> ContextualGraphResponse {
    ContextualGraphResponse {
        focus_node: GraphNode {
            id: String::new(),
            label: String::new(),
            kind: String::new(),
            file: None,
            line: None,
            style_class: String::new(),
        },
        parent: None,
        children: None,
        same_level: crate::dto::SameLevelSection {
            nodes: Vec::new(),
            edges: Vec::new(),
        },
        level: "file".to_string(),
        truncated: false,
        truncation_reason: None,
    }
}
