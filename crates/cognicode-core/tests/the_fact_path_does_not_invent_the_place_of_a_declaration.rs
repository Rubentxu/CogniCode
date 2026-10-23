//! The fact path must not lose a declaration, nor invent where it lives.
//!
//! Measured: the readers of the fact path read identities with the symbol
//! grammar alone. An identity that a reader of infrastructure files wrote
//! did not fit, and each reader answered with something else: the fact was
//! dropped, the file was left empty, or the name came back as the file the
//! thing lives in. A declaration is a node of the graph, and a graph that
//! places it nowhere is a graph that lost it.
//!
//! The law this file pins:
//!
//! - a declaration keeps the name its identity carries, and
//! - a call to a declaration resolves, instead of falling out of the graph.

#![cfg(feature = "evidence-kernel")]

use cognicode_core::domain::evidence_kernel::SymbolKindDetail;
use cognicode_core::domain::evidence_kernel::continuity::SnapshotEntityView;
use cognicode_core::domain::evidence_kernel::fact::{
    Fact, FactValue, ProducerKind, ProvenanceRecord,
};
use cognicode_core::domain::evidence_kernel::ids::{EntityId, FactId, SnapshotId};
use cognicode_core::domain::evidence_kernel::relation::RelationKind;
use cognicode_core::domain::value_objects::{Provenance, SymbolKind};
use cognicode_core::infrastructure::graph::CallGraphProjection;

const SNAPSHOT: SnapshotId = SnapshotId::new(1);

/// The identity a reader of infrastructure files writes for a resource.
const DECLARATION: &str = "tf:main.tf:aws_s3_bucket.app_bucket";

/// The identity a symbol writes for itself.
const CALLER: &str = "src/lib.rs:caller:1";

/// A `core:defines` fact: the subject and the object carry the identity.
fn define_fact(id: u64, identity: &str) -> Fact {
    Fact::new(
        FactId::new(id),
        EntityId::new(id),
        RelationKind::try_new("core:defines").expect("valid predicate"),
        FactValue::Text(identity.to_string()),
        SNAPSHOT,
        ProvenanceRecord::new(
            Provenance::Extracted,
            ProducerKind::DeterministicAnalyzer,
            Some(SymbolKindDetail::encode(SymbolKind::Declaration)),
        ),
    )
    .expect("deterministic producer")
}

/// A `core:calls` fact: the object is the name of the callee.
fn call_fact(id: u64, subject: u64, callee: &str) -> Fact {
    Fact::new(
        FactId::new(id),
        EntityId::new(subject),
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

#[test]
fn the_continuity_view_names_the_declaration_it_reads() {
    let facts = vec![define_fact(1, DECLARATION)];

    let view = SnapshotEntityView::from_facts(&facts, SNAPSHOT);
    let entity = view
        .entities
        .get(&EntityId::new(1))
        .expect("the declared entity is in the view");

    assert_eq!(
        entity.name, "aws_s3_bucket.app_bucket",
        "the view must name the declaration, not the file it lives in"
    );
    assert_eq!(
        entity.fqn, DECLARATION,
        "the identity is kept as it was written"
    );
}

#[test]
fn a_call_to_a_declaration_resolves_instead_of_falling_out() {
    let facts = vec![
        define_fact(1, DECLARATION),
        define_fact(2, CALLER),
        call_fact(3, 2, "aws_s3_bucket.app_bucket"),
    ];

    let projection = CallGraphProjection::from_facts(&facts);

    assert_eq!(
        projection.unresolved_edges(),
        0,
        "the call to a declaration must resolve, not fall out of the graph"
    );
    assert_eq!(
        projection.graph().edge_count(),
        1,
        "the call must be in the graph"
    );
}
