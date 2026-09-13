//! Tree-sitter extraction adapter (E37 design D1).
//!
//! Converts one `extract_file` [`ExtractionResult`] into canonical relation
//! observations, reusing the extractor's output verbatim:
//!
//! - symbol nodes → `(S, core:defines, Text(fqn))` with `kind=<SymbolKind>`
//!   riding the fact's `provenance.detail` (serde name);
//! - `dependency.contains` edges → `(F, core:contains, Text(fqn))`;
//! - `dependency.calls` edges → `(S, core:calls, Text(callee))`;
//! - `dependency.imports` edges → `(F, core:imports, Text(module))`;
//! - `dependency.references` edges → `(S, core:references, Text(name))`.
//!
//! Producer: `DeterministicAnalyzer` (design D1 authority table).

use crate::application::fact_bridge::batch_builder::FactBatchBuilder;
use crate::application::ingest::types::{ExtractionResult, TargetRef};
use crate::domain::evidence_kernel::fact::ProducerKind;
use crate::domain::evidence_kernel::symbol_kind_detail::SymbolKindDetail;
use crate::domain::value_objects::{NodeKind, SymbolKind};

/// Producer stamped on every tree-sitter-derived observation (design D1).
const PRODUCER: ProducerKind = ProducerKind::DeterministicAnalyzer;

/// Adds every relation derivable from `result` into `builder`.
///
/// Failed extractions contribute nothing (error isolation, ADR-023); edge
/// kinds outside the canonical mapping are skipped so the bridge never
/// emits a predicate outside the registered `core:*` set (design D2).
pub fn collect(builder: &mut FactBatchBuilder, result: &ExtractionResult) {
    if result.error.is_some() {
        return;
    }

    // Definition records: every non-file symbol node asserts its identity.
    for node in &result.nodes {
        let NodeKind::Symbol(kind) = node.kind else {
            continue;
        };
        if kind == SymbolKind::File {
            continue; // files carry no defines fact (fact grammar, design D1)
        }
        let fqn = node.id.as_str();
        builder
            .add_observation(
                fqn,
                super::relation("core:defines"),
                fqn,
                PRODUCER,
                // `kind=<SerdeName>` via the single codec (E38.1 CP-2).
                Some(SymbolKindDetail::encode(kind)),
            )
            .expect("DeterministicAnalyzer is never rejected");
    }

    // Relational facts: extractor edges mapped onto canonical predicates.
    for edge in &result.edges {
        let predicate = match edge.kind.as_str() {
            "dependency.contains" => "core:contains",
            "dependency.calls" => "core:calls",
            "dependency.imports" => "core:imports",
            "dependency.references" => "core:references",
            _ => continue,
        };
        let object = match &edge.target_ref {
            TargetRef::Resolved(id) => id.as_str(),
            TargetRef::Unresolved(name) => name.as_str(),
        };
        builder
            .add_observation(
                edge.source.as_str(),
                super::relation(predicate),
                object,
                PRODUCER,
                None,
            )
            .expect("DeterministicAnalyzer is never rejected");
    }
}

// E38.1 CP-2: the former local `symbol_kind_name` serde-name table moved
// into the single codec (`SymbolKindDetail`); this site routes through
// `SymbolKindDetail::encode`, which stays exhaustive over `SymbolKind`.

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::application::ingest::extractor::extract_file;
    use crate::application::ingest::types::{ExtractionEdge, ExtractionResult, TargetRef};
    use crate::domain::aggregates::{GraphNode, NodeId};
    use crate::domain::evidence_kernel::fact::{FactValue, ProducerKind};
    use crate::domain::evidence_kernel::ids::{EntityId, SnapshotId};
    use crate::domain::value_objects::{NodeKind, Provenance, SymbolKind};
    use crate::infrastructure::parser::language_config::RUST_CONFIG;

    use super::*;

    const SNAPSHOT: SnapshotId = SnapshotId::new(1);

    fn edge(source: &str, target: TargetRef, kind: &str) -> ExtractionEdge {
        ExtractionEdge {
            source: source.to_string(),
            target_ref: target,
            kind: kind.to_string(),
            provenance: Provenance::Extracted,
            confidence: 1.0,
            line: None,
        }
    }

    fn node(id: &str, kind: NodeKind) -> GraphNode {
        GraphNode::builder(NodeId::new(id), kind).build()
    }

    /// A synthetic extraction covering all four mapped edge kinds plus a
    /// file node and one function symbol.
    fn sample_extraction() -> ExtractionResult {
        let file_node = node("src/lib.rs", NodeKind::Symbol(SymbolKind::File));
        let greet = node("src/lib.rs:greet:1", NodeKind::Symbol(SymbolKind::Function));
        let edges = vec![
            edge(
                "src/lib.rs",
                TargetRef::Resolved("src/lib.rs:greet:1".to_string()),
                "dependency.contains",
            ),
            edge(
                "src/lib.rs:greet:1",
                TargetRef::Unresolved("format".to_string()),
                "dependency.calls",
            ),
            edge(
                "src/lib.rs",
                TargetRef::Unresolved("std::collections".to_string()),
                "dependency.imports",
            ),
            edge(
                "src/lib.rs:greet:1",
                TargetRef::Unresolved("HashMap".to_string()),
                "dependency.references",
            ),
            // Unmapped kinds are skipped by the adapter.
            edge(
                "src/lib.rs:greet:1",
                TargetRef::Unresolved("ignored".to_string()),
                "dependency.annotated_by",
            ),
        ];
        ExtractionResult::ok(
            PathBuf::from("src/lib.rs"),
            "hash".to_string(),
            vec![file_node, greet],
            edges,
        )
    }

    /// Every extractor surface maps onto its canonical predicate with the
    /// right subject/object; the file node emits no defines fact and the
    /// unmapped edge kind is skipped.
    #[test]
    fn maps_extraction_to_canonical_predicates() {
        let mut builder = FactBatchBuilder::new(SNAPSHOT);
        builder.add_extraction(&sample_extraction());
        let facts = builder.finish();

        let find = |predicate: &str, object: &str| {
            facts
                .iter()
                .find(|f| {
                    f.predicate.as_str() == predicate
                        && matches!(&f.object, FactValue::Text(t) if t == object)
                })
                .unwrap_or_else(|| panic!("{predicate} → {object} fact missing"))
        };

        // defines: subject is the symbol entity, detail carries kind=<K>.
        let defines = find("core:defines", "src/lib.rs:greet:1");
        assert_eq!(
            defines.provenance.detail.as_deref(),
            Some("kind=Function"),
            "symbol kind rides provenance.detail (serde name)"
        );
        assert_eq!(
            defines.provenance.producer,
            ProducerKind::DeterministicAnalyzer
        );

        // contains: file → symbol fqn. Entities follow sorted subject
        // strings: "src/lib.rs" → 1, "src/lib.rs:greet:1" → 2.
        let contains = find("core:contains", "src/lib.rs:greet:1");
        assert_eq!(contains.subject, EntityId::new(1), "file entity is first");
        assert_eq!(defines.subject, EntityId::new(2), "symbol entity is second");

        // calls / imports / references.
        find("core:calls", "format");
        find("core:imports", "std::collections");
        find("core:references", "HashMap");

        // The file entity never gets a defines fact; 1 defines + 4 mapped
        // edges exist, the unmapped edge kind is skipped.
        assert_eq!(facts.len(), 5);
        assert!(
            facts
                .iter()
                .all(|f| f.provenance.producer == ProducerKind::DeterministicAnalyzer)
        );
    }

    /// A failed extraction contributes nothing (error isolation).
    #[test]
    fn failed_extraction_contributes_no_records() {
        let failed = ExtractionResult::failed(
            PathBuf::from("broken.rs"),
            "hash".to_string(),
            "parse failure".to_string(),
        );
        let mut builder = FactBatchBuilder::new(SNAPSHOT);
        builder.add_extraction(&failed);
        assert!(builder.finish().is_empty());
    }

    /// End-to-end over a real `extract_file` run: a Rust source with a use
    /// statement, a struct, and calls produces defines/contains/calls facts
    /// with correct `kind=` details.
    #[test]
    fn real_extraction_yields_canonical_facts() {
        let source = r#"
use std::collections::HashMap;

pub struct Widget;

pub fn build() -> Widget {
    let map: HashMap<String, String> = HashMap::new();
    Widget
}
"#;
        let result = extract_file(
            &RUST_CONFIG,
            std::path::Path::new("synthetic.rs"),
            source,
            "hash",
        );
        assert!(result.is_ok(), "extraction failed: {:?}", result.error);

        let mut builder = FactBatchBuilder::new(SNAPSHOT);
        builder.add_extraction(&result);
        let facts = builder.finish();

        assert!(!facts.is_empty());
        assert!(
            facts.iter().any(|f| f.predicate.as_str() == "core:defines"
                && f.provenance.detail.as_deref() == Some("kind=Struct")),
            "struct definition must carry kind=Struct"
        );
        assert!(
            facts.iter().any(|f| f.predicate.as_str() == "core:defines"
                && f.provenance.detail.as_deref() == Some("kind=Function")),
            "function definition must carry kind=Function"
        );
        assert!(
            facts
                .iter()
                .any(|f| f.predicate.as_str() == "core:contains"),
            "file containment must be recorded"
        );
        assert!(
            facts.iter().any(|f| f.predicate.as_str() == "core:calls"
                && matches!(&f.object, FactValue::Text(t) if t == "new")),
            "call edges must map to core:calls"
        );
    }
}
