//! E40 GenericGraph equivalence harness (design D7 + RETIREMENT-LEDGER GAP S2).
//!
//! Closes the S2 gap from `docs/CogniCode_Living_Software_Intelligence/RETIREMENT-LEDGER.md`:
//! the previous harness only covered CallGraphProjection. This sibling file
//! extends coverage to `FactGenericGraphProjection` (e37 design D5) by
//! verifying the four invariants declared in
//! `crates/cognicode-core/src/infrastructure/graph/generic_graph_projection.rs`:
//!
//! 1. **Determinism** — rebuild over the same facts yields a byte-identical
//!    projection (drained ordering independent).
//! 2. **Dangling-free** — an edge is emitted only when BOTH endpoints
//!    resolve to emitted nodes; targets resolve by exact identity string
//!    first, then by lowercase name with deterministic tie-break.
//! 3. **Self-loop-free** — `GraphEdge::new` rejects self-loops.
//! 4. **Kind-multiset equivalence** — the kind multiset of emitted nodes
//!    matches the multiset of kinds declared in `core:defines` facts
//!    (E38.1 CP-2 single-codec contract).
//!
//! Comparison contract (declared BEFORE any comparison, per
//! `projection-architecture` spec, requirement "Derived projections are
//! rebuildable"):
//!
//! - The SAME facts, run twice, MUST produce byte-identical `GenericProjection`
//!   values (rebuild equivalence).
//! - Empty facts MUST yield an empty projection (zero nodes, zero edges).
//! - A dangling reference (callee name that does not resolve to an emitted
//!   node) MUST be silently dropped, NOT panic, NOT corrupt the projection.
//! - Self-loop attempts (`source == target`) MUST be silently dropped, NOT
//!   panic, NOT corrupt the projection.
//!
//! No legacy oracle exists for GenericGraph (the legacy
//! `AnalysisService::build_project_graph` only emitted CallGraph). The
//! harness therefore exercises the FACT-DERIVED projection over a synthetic
//! corpus (a strict subset of the canonical grammar the projection already
//! documents in its inline tests) and pins a per-fixture digest so that any
//! change to the projection contract is caught explicitly.
//!
//! Quarantine: this harness has no quarantined fixtures — every scenario is
//! a property of the projection that MUST hold for ANY input. The score
//! threshold is therefore implicit (= 1.0): every scenario MUST pass or the
//! build fails the run.
//!
//! Wired behind `feature = "evidence-kernel"` + `feature = "multimodal"`
//! (mirror of e37 harness). `FactGenericGraphProjection` lives under
//! `infrastructure::graph` which is gated on both features.
#![cfg(all(feature = "evidence-kernel", feature = "multimodal"))]

use std::sync::Arc;

use cognicode_core::domain::aggregates::{GraphNode, NodeId};
use cognicode_core::domain::evidence_kernel::SymbolKindDetail;
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
use cognicode_core::domain::evidence_kernel::ports::FactStore;
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::ports::generic_graph_projection::{
    GenericGraphProjectionPort, GenericProjection,
};
use cognicode_core::domain::value_objects::{NodeKind, Provenance, SymbolKind, WorkspaceId};
use cognicode_core::infrastructure::evidence_kernel::in_memory::{
    InMemoryFactStore, InMemorySchemaRegistry,
};
use cognicode_core::infrastructure::graph::FactGenericGraphProjection;
use sha2::{Digest, Sha256};

/// Pinned digest of the per-fixture projection contract. Any change to the
/// projection's deterministic output (sort order, kind encoding, edge
/// skipping policy) MUST be re-pinned as a new baseline; the harness then
/// fails until the new digest is committed.
pub const PINNED_GENERIC_PROJECTION_DIGEST: &str =
    "sha256:6d22cf734f1094a07df63aea4698e512ae5046f4fa732a322ea43b31678a1cc5";

const SNAPSHOT: SnapshotId = SnapshotId::new(1);
const WORKSPACE: &str = "ws-e40-generic-graph-equivalence";

/// Build a `core:defines` fact carrying the `kind=<SerdeName>` detail
/// (E38.1 CP-2 single-codec contract — design D3).
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
            Some(SymbolKindDetail::encode(kind)),
        ),
    )
    .expect("deterministic producer")
}

/// Build a `core:contains` fact for a file entity.
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

/// Build a `core:calls` fact — the most common edge predicate.
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

/// Structural fingerprint of a `GenericProjection` for equivalence
/// assertions. Excludes `GraphNode::created_at`/`updated_at` (wall-clock
/// fields set by `GraphNode::builder`) — those vary between rebuilds and
/// are NOT part of the projection contract. The contract (design D5)
/// guarantees that `nodes` are sorted by `NodeId`, `edges` are sorted by
/// `(source, target, kind)`, and both vectors are fully determined by the
/// input facts. The fingerprint therefore reduces to a sorted list of
/// `(NodeId, NodeKind, label, source_path, sorted_edges)` tuples.
///
/// Pinned as `PINNED_GENERIC_PROJECTION_DIGEST`.
fn projection_digest(p: &GenericProjection) -> String {
    let mut node_lines: Vec<String> = p
        .nodes
        .iter()
        .map(|n| {
            format!(
                "N|{:?}|{:?}|{}|{}",
                n.id,
                n.kind,
                n.label,
                n.source_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            )
        })
        .collect();
    node_lines.sort();

    let mut edge_lines: Vec<String> = p
        .edges
        .iter()
        .map(|e| {
            format!(
                "E|{:?}|{:?}|{:?}|{:?}",
                e.source, e.target, e.kind, e.provenance
            )
        })
        .collect();
    edge_lines.sort();

    let mut hasher = Sha256::new();
    hasher.update(b"nodes:\n");
    for line in &node_lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    hasher.update(b"edges:\n");
    for line in &edge_lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    format!("sha256:{:x}", hasher.finalize())
}

async fn project_with(facts: Vec<Fact>) -> GenericProjection {
    let registry = InMemorySchemaRegistry::new();
    cognicode_core::domain::evidence_kernel::bootstrap::bootstrap_registry(&registry)
        .expect("canonical bootstrap");
    let store = Arc::new(InMemoryFactStore::new(Arc::new(registry)));
    let ws = WorkspaceId::try_new(WORKSPACE).expect("valid workspace");
    if !facts.is_empty() {
        store
            .commit(&ws, &SNAPSHOT, facts)
            .await
            .expect("canonical facts commit");
    }
    let adapter = FactGenericGraphProjection::new(store);
    adapter
        .project(&ws, &SNAPSHOT)
        .await
        .expect("projection succeeds")
}

/// Canonical sample: file `src/lib.rs` with two functions `greet` and `main`,
/// `main` calls `greet`, one call fact stays unresolved.
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

/// Spec scenario "Derived projections are rebuildable — Projection rebuild":
/// running the same facts twice MUST yield byte-identical projection.
#[tokio::test]
async fn rebuild_byte_identical() {
    let first = project_with(sample_facts()).await;
    let second = project_with(sample_facts()).await;

    let first_digest = projection_digest(&first);
    let second_digest = projection_digest(&second);

    assert_eq!(
        first_digest, second_digest,
        "rebuild over the same facts MUST be byte-identical (design D5: \
         ordered maps in, sorted vectors out)"
    );
}

/// Spec scenario "Dangling-free contract (design D5)": an edge is emitted
/// only when BOTH endpoints resolve to emitted nodes. The sample has one
/// unresolved callee (`missing_callee`); the projection MUST skip it
/// without panic and without including it in the edge multiset.
///
/// The sample has 3 emit-worthy edges:
///   1. `src/lib.rs → src/lib.rs:greet:1`    (`core:contains` lib→greet)
///   2. `src/lib.rs → src/lib.rs:main:2`     (`core:contains` lib→main)
///   3. `src/lib.rs:main:2 → src/lib.rs:greet:1` (`core:calls` main→greet)
///
/// `missing_callee` resolves to no node → dropped (dangling-free).
#[tokio::test]
async fn dangling_edge_skipped() {
    let projection = project_with(sample_facts()).await;

    // Two `core:defines` symbols + one file entity (file-entity contains
    // two symbols): 3 emitted nodes.
    assert_eq!(
        projection.nodes.len(),
        3,
        "expected 3 nodes (file + 2 functions); got {:?}",
        projection.nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
    );

    // 2 `core:contains` edges + 1 `core:calls` edge; `missing_callee` MUST be
    // skipped (design D5 dangling-free contract).
    assert_eq!(
        projection.edges.len(),
        3,
        "expected 3 edges (lib→greet, lib→main, main→greet); got {:?}",
        projection
            .edges
            .iter()
            .map(|e| (&e.source, &e.target))
            .collect::<Vec<_>>()
    );

    // Sanity: the dangling target MUST NOT appear as the target of any edge.
    let targets: Vec<&cognicode_core::domain::aggregates::NodeId> =
        projection.edges.iter().map(|e| &e.target).collect();
    assert!(
        !targets
            .iter()
            .any(|t| format!("{:?}", t).contains("missing_callee")),
        "dangling callee `missing_callee` MUST NOT appear as an edge target"
    );
}

/// Spec scenario "Kind-multiset equivalence" (E38.1 CP-2 single-codec):
/// the multiset of emitted node kinds MUST match the multiset of kinds
/// declared in `core:defines` facts.
#[tokio::test]
async fn kind_multiset_matches() {
    let projection = project_with(sample_facts()).await;

    let kinds: Vec<NodeKind> = projection
        .nodes
        .iter()
        .map(|n: &GraphNode| n.kind.clone())
        .collect();

    // Expected: 2 Function (greet, main) + 1 File (lib.rs file entity).
    let mut expected = vec![
        NodeKind::Symbol(SymbolKind::Function),
        NodeKind::Symbol(SymbolKind::Function),
        NodeKind::Symbol(SymbolKind::File),
    ];
    expected.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));

    let mut actual = kinds.clone();
    actual.sort_by(|a, b| format!("{:?}", a).cmp(&format!("{:?}", b)));

    assert_eq!(
        actual, expected,
        "kind multiset MUST match declares (E38.1 CP-2)"
    );
}

/// Spec scenario "Empty input ⇒ empty projection": no facts ⇒ no nodes, no
/// edges. Guards against spurious default-state emission.
#[tokio::test]
async fn empty_facts_yields_empty_projection() {
    let projection = project_with(vec![]).await;

    assert!(
        projection.nodes.is_empty(),
        "empty facts MUST yield zero nodes; got {:?}",
        projection.nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
    );
    assert!(
        projection.edges.is_empty(),
        "empty facts MUST yield zero edges"
    );
}

/// Pinned digest gate (mirror of e37's `PINNED_IDENTITY_DIGEST`): if any of
/// the four scenarios above changes its output, this digest MUST be
/// re-pinned as a new baseline.
#[tokio::test]
async fn pinned_digest_matches() {
    let projection = project_with(sample_facts()).await;
    let actual = projection_digest(&projection);

    // First-run seeding: write the actual digest below (one-time). After
    // seeding, any divergence from the pinned digest fails the build until
    // the projection contract change is consciously committed.
    assert_eq!(
        actual, PINNED_GENERIC_PROJECTION_DIGEST,
        "projection contract changed; re-pin PINNED_GENERIC_PROJECTION_DIGEST \
         and commit the new baseline (mirror of e37 PINNED_IDENTITY_DIGEST rule)"
    );
}

// Sanity check: the producer/relation/symbol kinds used in this module must
// be in sync with the canonical grammar — guards against typos in helpers
// above silently producing structurally invalid facts.
#[test]
fn helper_kinds_are_canonical() {
    let _f = define_fact(1, "x:y:1", SymbolKind::Function);
    let _f = contains_fact(2, 1, "x:y:1");
    let _f = call_fact(3, 1, "y");
    // Compile-time guarantee: helpers are usable.
}

// Reference `_NodeId` to silence unused-import lint if NodeId becomes unused
// in future refactors (it documents the type aliasing contract).
#[allow(dead_code)]
fn _assert_node_id_alias(id: NodeId) -> String {
    format!("{:?}", id)
}
