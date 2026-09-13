//! Fact-derived generic graph adapter (E37 design D5).
//!
//! [`FactGenericGraphProjection`] implements the dual-gated
//! [`GenericGraphProjectionPort`](crate::domain::ports::generic_graph_projection::GenericGraphProjectionPort)
//! by reading the pinned snapshot's facts through the kernel [`FactStore`]
//! port and mapping them onto the existing `GraphNode`/`GraphEdge` values —
//! the output contains NO kernel types, so consumers stay FactStore-ignorant
//! (spec `generic-graph-projection`, "Consumer needs only emitted types").
//!
//! ## Fact → node/edge mapping
//!
//! - **Symbol nodes**: every entity carrying a `core:defines` record becomes
//!   a `GraphNode` with `id = fqn` (the record's object), `kind` parsed from
//!   the `kind=<SerdeName>` provenance detail, `label` = the FQN name
//!   segment and `source_path` = the FQN file segment.
//! - **File nodes**: entities that are `core:contains` subjects without a
//!   `core:defines` record become `Symbol(File)` nodes; their identity
//!   string is the file segment derived from their contained symbols' FQNs.
//! - **Edges**: every relational fact (`core:calls`, `core:imports`,
//!   `core:contains`, `core:inherits`, `core:references`) becomes a
//!   `GraphEdge` with `EdgeKind::Dependency(...)` (predicate →
//!   `DependencyType`), confidence `1.0`, and the fact's provenance class.
//! - **Dangling-free contract (design D5)**: an edge is emitted only when
//!   BOTH endpoints resolve to emitted nodes. Targets are resolved by exact
//!   identity-string match first, then by lowercase name with the same
//!   deterministic lexicographic tie-break as
//!   `CallGraphProjection::from_facts`. Unresolved-callee edges are skipped;
//!   self-loops are skipped through the `GraphEdge::new` error. Bare module
//!   names on `core:imports` facts are not entities, so those edges resolve
//!   only when the name coincides with an emitted node.
//!
//! Rebuild is deterministic by construction: ordered maps during
//! construction, nodes sorted by id, edges sorted by
//! `(source, target, kind)`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::aggregates::{GraphEdge, GraphNode, NodeId};
use crate::domain::evidence_kernel::fact::{Fact, FactValue};
use crate::domain::evidence_kernel::ids::SnapshotId;
use crate::domain::evidence_kernel::ports::{FactStore, KernelError};
use crate::domain::ports::generic_graph_projection::{
    GenericGraphProjectionPort, GenericProjection,
};
use crate::domain::value_objects::{
    DependencyType, EdgeKind, NodeKind, Provenance, SymbolKind, WorkspaceId,
};

use super::call_graph_projection::{parse_fqn, symbol_kind_from_detail};

/// One deduplicated relational edge before GraphNode construction:
/// `(source, target, predicate)` → provenance class of the observed fact.
type EdgeKey = (String, String, String);

/// Predicate → `DependencyType` mapping for the canonical `core:*` set.
/// `core:defines` is an identity record, not a relational edge, and never
/// reaches this table.
fn dependency_type(predicate: &str) -> Option<DependencyType> {
    match predicate {
        "core:calls" => Some(DependencyType::Calls),
        "core:imports" => Some(DependencyType::Imports),
        "core:contains" => Some(DependencyType::Contains),
        "core:inherits" => Some(DependencyType::Inherits),
        "core:references" => Some(DependencyType::References),
        _ => None,
    }
}

/// In-memory generic graph projection over a kernel [`FactStore`] (E37
/// design D5).
pub struct FactGenericGraphProjection {
    store: Arc<dyn FactStore>,
}

impl FactGenericGraphProjection {
    /// Creates an adapter reading facts from `store`.
    pub fn new(store: Arc<dyn FactStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl GenericGraphProjectionPort for FactGenericGraphProjection {
    async fn project(
        &self,
        ws: &WorkspaceId,
        snap: &SnapshotId,
    ) -> Result<GenericProjection, KernelError> {
        let facts = self.store.facts_in_snapshot(ws, snap).await?;
        Ok(build_generic_projection(&facts))
    }
}

/// Pure fact → generic-graph mapping shared by the adapter, tests and
/// benchmarks. Deterministic: ordered maps in, sorted vectors out.
pub(crate) fn build_generic_projection(facts: &[Fact]) -> GenericProjection {
    // ── Pass 1: entity identity strings ────────────────────────────────
    // Symbol entities self-assert through `core:defines` (subject → fqn).
    // File entities are contains-subjects; their identity string is the
    // file segment of the contained symbols' FQNs.
    let mut identity: BTreeMap<u64, String> = BTreeMap::new();
    let mut kinds: BTreeMap<u64, SymbolKind> = BTreeMap::new();
    let mut defines_subjects: BTreeSet<u64> = BTreeSet::new();
    for fact in facts {
        if fact.predicate.as_str() == "core:defines" {
            let FactValue::Text(fqn) = &fact.object else {
                continue;
            };
            defines_subjects.insert(fact.subject.get());
            identity.insert(fact.subject.get(), fqn.clone());
            kinds.insert(
                fact.subject.get(),
                symbol_kind_from_detail(fact.provenance.detail.as_deref()),
            );
        }
    }
    for fact in facts {
        if fact.predicate.as_str() != "core:contains" {
            continue;
        }
        let FactValue::Text(fqn) = &fact.object else {
            continue;
        };
        let Some((file, _, _)) = parse_fqn(fqn) else {
            continue;
        };
        identity.entry(fact.subject.get()).or_insert(file);
    }

    // ── Nodes ──────────────────────────────────────────────────────────
    // id → (kind, label, source_path), keyed by id string for determinism.
    let mut nodes: BTreeMap<String, (NodeKind, String, PathBuf)> = BTreeMap::new();
    let mut symbol_names: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (entity, id_string) in &identity {
        if defines_subjects.contains(entity) {
            let kind = kinds.get(entity).copied().unwrap_or(SymbolKind::Unknown);
            let (file, name, _) =
                parse_fqn(id_string).unwrap_or((String::new(), id_string.clone(), 1));
            symbol_names
                .entry(name.to_lowercase())
                .or_default()
                .insert(id_string.clone());
            nodes.insert(
                id_string.clone(),
                (NodeKind::Symbol(kind), name, PathBuf::from(file)),
            );
        } else {
            let label = std::path::Path::new(id_string)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| id_string.clone());
            nodes.insert(
                id_string.clone(),
                (
                    NodeKind::Symbol(SymbolKind::File),
                    label,
                    PathBuf::from(id_string.clone()),
                ),
            );
        }
    }

    // ── Edges ──────────────────────────────────────────────────────────
    // Both endpoints must resolve to emitted nodes (dangling-free contract,
    // design D5). Targets resolve by exact identity string first, then by
    // lowercase name with the deterministic lexicographic tie-break.
    let mut edge_keys: BTreeMap<EdgeKey, Provenance> = BTreeMap::new();
    for fact in facts {
        if dependency_type(fact.predicate.as_str()).is_none() {
            continue; // defines (identity) and unknown predicates are not edges
        }
        let FactValue::Text(target_text) = &fact.object else {
            continue;
        };
        let Some(source_string) = identity.get(&fact.subject.get()) else {
            continue; // endpoint not an emitted node → skip (dangling-free)
        };
        let target_string = if nodes.contains_key(target_text) {
            Some(target_text.clone())
        } else {
            symbol_names
                .get(&target_text.to_lowercase())
                .and_then(|candidates| candidates.iter().next())
                .cloned()
        };
        let Some(target_string) = target_string else {
            continue; // unresolved callee/parent/reference: skipped (design D5)
        };
        edge_keys.insert(
            (
                source_string.clone(),
                target_string,
                fact.predicate.as_str().to_string(),
            ),
            fact.provenance.class,
        );
    }

    let edges: Vec<GraphEdge> = edge_keys
        .into_iter()
        .filter_map(|((source, target, predicate), class)| {
            let kind = EdgeKind::Dependency(dependency_type(&predicate)?);
            GraphEdge::new(NodeId::new(source), NodeId::new(target), kind, class, 1.0).ok() // self-loops are rejected by GraphEdge::new and skipped
        })
        .collect();

    let nodes: Vec<GraphNode> = nodes
        .into_iter()
        .map(|(id, (kind, label, source_path))| {
            GraphNode::builder(NodeId::new(id), kind)
                .label(label)
                .source_path(source_path)
                .build()
        })
        .collect();

    GenericProjection { nodes, edges }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::domain::evidence_kernel::bootstrap::bootstrap_registry;
    use crate::domain::evidence_kernel::fact::{Fact, FactValue, ProducerKind, ProvenanceRecord};
    use crate::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
    use crate::domain::evidence_kernel::ports::FactStore;
    use crate::domain::evidence_kernel::relation::RelationKind;
    use crate::domain::ports::generic_graph_projection::GenericGraphProjectionPort;
    use crate::domain::value_objects::{NodeKind, Provenance, SymbolKind, WorkspaceId};
    use crate::infrastructure::evidence_kernel::in_memory::{
        InMemoryFactStore, InMemorySchemaRegistry,
    };

    use super::*;

    const SNAPSHOT: SnapshotId = SnapshotId::new(1);

    /// A committed store + workspace for adapter-level tests: registers the
    /// canonical vocabulary and commits `facts` into `SNAPSHOT`.
    async fn store_with(facts: Vec<Fact>) -> (Arc<InMemoryFactStore>, WorkspaceId) {
        let registry = InMemorySchemaRegistry::new();
        bootstrap_registry(&registry).expect("canonical bootstrap");
        let store = Arc::new(InMemoryFactStore::new(Arc::new(registry)));
        let ws = WorkspaceId::try_new("ws-e37").expect("valid workspace");
        if !facts.is_empty() {
            store
                .commit(&ws, &SNAPSHOT, facts)
                .await
                .expect("canonical facts commit");
        }
        (store, ws)
    }

    fn define_fact(id: u64, fqn: &str, kind: SymbolKind) -> Fact {
        Fact::new(
            FactId::new(id),
            EntityId::new(id),
            RelationKind::try_new("core:defines").expect("valid predicate"),
            FactValue::Text(fqn.to_string()),
            SNAPSHOT,
            ProvenanceRecord::new(
                Provenance::Extracted,
                ProducerKind::DeterministicAnalyzer,
                Some(format!("kind={}", serde_kind_name(kind))),
            ),
        )
        .expect("deterministic producer")
    }

    fn contains_fact(id: u64, file_entity: u64, fqn: &str) -> Fact {
        Fact::new(
            FactId::new(id),
            EntityId::new(file_entity),
            RelationKind::try_new("core:contains").expect("valid predicate"),
            FactValue::Text(fqn.to_string()),
            SNAPSHOT,
            ProvenanceRecord::new(
                Provenance::Extracted,
                ProducerKind::DeterministicAnalyzer,
                None,
            ),
        )
        .expect("deterministic producer")
    }

    fn call_fact(id: u64, subject_entity: u64, callee: &str) -> Fact {
        Fact::new(
            FactId::new(id),
            EntityId::new(subject_entity),
            RelationKind::try_new("core:calls").expect("valid predicate"),
            FactValue::Text(callee.to_string()),
            SNAPSHOT,
            ProvenanceRecord::new(
                Provenance::Extracted,
                ProducerKind::DeterministicAnalyzer,
                None,
            ),
        )
        .expect("deterministic producer")
    }

    /// The full file-entity fact set for `src/lib.rs` with two functions —
    /// `main` calls `greet`; one call fact stays unresolved (matches the
    /// bridge's canonical grammar).
    fn sample_facts() -> Vec<Fact> {
        vec![
            contains_fact(1, 10, "src/lib.rs:greet:1"),
            define_fact(2, "src/lib.rs:greet:1", SymbolKind::Function),
            call_fact(3, 2, "missing_callee"),
            contains_fact(4, 10, "src/lib.rs:main:2"),
            define_fact(5, "src/lib.rs:main:2", SymbolKind::Function),
            call_fact(6, 5, "greet"),
        ]
    }

    /// The serde name of a `SymbolKind` (mirrors the bridge convention).
    fn serde_kind_name(kind: SymbolKind) -> &'static str {
        match kind {
            SymbolKind::Function => "Function",
            SymbolKind::File => "File",
            other => unreachable!("test helper covers {other:?} via full match in impl"),
        }
    }

    /// Spec scenario "Facts map to nodes and edges": emitted nodes
    /// correspond to fact entity values and edges to fact relations with
    /// matching endpoints — including the file node and the resolved call;
    /// the unresolved call is skipped (dangling-free contract).
    #[tokio::test]
    async fn facts_map_to_nodes_and_edges() {
        let (store, ws) = store_with(sample_facts()).await;
        let projection = FactGenericGraphProjection::new(store.clone());
        let out = projection
            .project(&ws, &SNAPSHOT)
            .await
            .expect("projection");

        // Nodes: file entity 10 (identity derived from the contains
        // objects) and symbol entities 2/5 (identity from their defines
        // records).
        assert_eq!(out.nodes.len(), 3, "one file node + two symbol nodes");
        let file_node = out
            .nodes
            .iter()
            .find(|n| n.id.as_str() == "src/lib.rs")
            .expect("file node");
        assert_eq!(file_node.kind, NodeKind::Symbol(SymbolKind::File));
        let symbol_node = out
            .nodes
            .iter()
            .find(|n| n.id.as_str() == "src/lib.rs:greet:1")
            .expect("symbol node");
        assert_eq!(symbol_node.kind, NodeKind::Symbol(SymbolKind::Function));
        assert_eq!(symbol_node.label, "greet");

        // Edges: contains (file → symbol fqns) and the RESOLVED call
        // (main → greet); the unresolved call is skipped.
        assert_eq!(out.edges.len(), 3, "two contains + one resolved call");
        let contains = out
            .edges
            .iter()
            .find(|e| {
                e.source.as_str() == "src/lib.rs" && e.target.as_str() == "src/lib.rs:greet:1"
            })
            .expect("contains edge");
        assert!(matches!(
            contains.kind,
            crate::domain::value_objects::EdgeKind::Dependency(DependencyType::Contains)
        ));
        let call = out
            .edges
            .iter()
            .find(|e| e.source.as_str() == "src/lib.rs:main:2")
            .expect("resolved call edge");
        assert_eq!(call.target.as_str(), "src/lib.rs:greet:1");
        assert!(matches!(
            call.kind,
            crate::domain::value_objects::EdgeKind::Dependency(DependencyType::Calls)
        ));
    }

    /// Spec scenario "Empty snapshot yields empty projection": a pinned
    /// snapshot with no committed facts emits nothing, without error.
    #[tokio::test]
    async fn empty_snapshot_yields_empty_projection() {
        let (store, ws) = store_with(Vec::new()).await;
        let projection = FactGenericGraphProjection::new(store.clone());
        let out = projection
            .project(&ws, &SNAPSHOT)
            .await
            .expect("projection");
        assert!(out.nodes.is_empty(), "no nodes for an empty snapshot");
        assert!(out.edges.is_empty(), "no edges for an empty snapshot");
    }

    /// Spec scenario "Clear and rebuild is equivalent": rebuilding from the
    /// same pinned facts (fresh store, same canonical batch) yields an
    /// equivalent projection. Nodes are compared on the stable fields (the
    /// aggregate stamps fresh timestamps per build); edges compare fully.
    #[tokio::test]
    async fn clear_and_rebuild_is_equivalent() {
        let (store, ws) = store_with(sample_facts()).await;
        let first = FactGenericGraphProjection::new(store.clone())
            .project(&ws, &SNAPSHOT)
            .await
            .expect("first projection");

        // "Clear" = drop the projection storage; "rebuild" = a fresh store
        // re-committed from the same pinned facts.
        let (store2, _ws2) = store_with(sample_facts()).await;
        let second = FactGenericGraphProjection::new(store2.clone())
            .project(&ws, &SNAPSHOT)
            .await
            .expect("rebuilt projection");

        let node_key = |p: &GenericProjection| -> Vec<(String, NodeKind, String)> {
            p.nodes
                .iter()
                .map(|n| (n.id.as_str().to_string(), n.kind.clone(), n.label.clone()))
                .collect()
        };
        assert_eq!(node_key(&first), node_key(&second), "rebuilt nodes equal");
        assert_eq!(first.edges, second.edges, "rebuilt edges equal");
    }

    /// Spec scenario "Consumer needs only emitted types": the output is
    /// exclusively `GraphNode`/`GraphEdge` — this consumer signature takes
    /// the emitted values and touches no FactStore type.
    #[tokio::test]
    async fn consumer_needs_only_emitted_types() {
        fn consumer(nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> usize {
            nodes.len() + edges.len()
        }
        let (store, ws) = store_with(sample_facts()).await;
        let out = FactGenericGraphProjection::new(store.clone())
            .project(&ws, &SNAPSHOT)
            .await
            .expect("projection");
        let GenericProjection { nodes, edges } = out;
        assert_eq!(consumer(nodes, edges), 6);
    }

    /// Determinism: the same fact set visited in a different order builds
    /// the same projection (ordered maps + sorted output, design D5).
    #[test]
    fn build_is_order_independent() {
        let mut facts = sample_facts();
        facts.reverse();
        let a = build_generic_projection(&sample_facts());
        let b = build_generic_projection(&facts);
        let node_ids = |p: &GenericProjection| -> Vec<String> {
            p.nodes.iter().map(|n| n.id.as_str().to_string()).collect()
        };
        let edge_keys = |p: &GenericProjection| -> Vec<(String, String, String)> {
            let mut keys: Vec<(String, String, String)> = p
                .edges
                .iter()
                .map(|e| (e.source.0.clone(), e.target.0.clone(), e.kind.to_string()))
                .collect();
            keys.sort();
            keys
        };
        assert_eq!(node_ids(&a), node_ids(&b));
        assert_eq!(edge_keys(&a), edge_keys(&b));
    }

    /// Self-loop call facts are skipped through the `GraphEdge::new` error
    /// (design D5): no edge, no panic.
    #[test]
    fn self_loop_call_is_skipped() {
        let facts = vec![
            define_fact(1, "src/lib.rs:main:1", SymbolKind::Function),
            call_fact(2, 1, "main"),
        ];
        let out = build_generic_projection(&facts);
        assert_eq!(out.nodes.len(), 1);
        assert!(
            out.edges.is_empty(),
            "a self-referencing call must not produce an edge"
        );
    }
}
